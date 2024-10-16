use anyhow::Result;
use qdrant_warp::constants::{COLLECTION, SECRETS};
use qdrant_warp::routes::add::{add, Add};
use qdrant_warp::routes::chat::chat;
use qdrant_warp::routes::chat_from::chat_from;
use qdrant_warp::routes::chats::chats;
use qdrant_warp::routes::chats_from::chats_from;
use qdrant_warp::routes::next_id::next_id;
use qdrant_warp::util::embedding;
use qdrant_warp::{
    app::{AppError, AppResult},
    constants::PRIVATE,
    qdrant::{qdrant_path, qdrant_post, qdrant_put},
    util::random_embedding,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shuttle_runtime::SecretStore;
use warp::{Filter, Reply};

#[shuttle_runtime::main]
async fn warp(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_warp::ShuttleWarp<(impl Reply,)> {
    let mut secrets_ = SECRETS.lock().await;
    *secrets_ = secrets;

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .allow_headers(vec!["Content-Type"]);

    let get_route = warp::path::end()
        .and(warp::get())
        .and(warp::query::<ItemQuery>())
        .and_then(handle_get);

    // let delete_route = warp::path::end()
    //     .and(warp::delete())
    //     .and(warp::query::<ItemQuery>())
    //     .and_then(handle_delete);

    // let set_route = warp::path::end()
    //     .and(warp::put())
    //     .and(warp::body::json::<Set>())
    //     .and_then(handle_set);

    let add = warp::path::end()
        .and(warp::post())
        .and(warp::body::json::<Add>())
        .and(warp::filters::addr::remote())
        .then(add);

    let search_route = warp::path("search")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<SearchQuery>())
        .and_then(handle_search);

    let routes = get_route
        // .or(delete_route)
        // .or(set_route)
        .or(add)
        .or(search_route)
        .or(search_route)
        .or(warp::path("i")
            .and(warp::path::end())
            .and(warp::get())
            .and_then(i_handler))
        .or(warp::path("groupsearch")
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::json::<GroupSearch>())
            .and_then(handle_group_search))
        .or(warp::path!("chats").and(warp::get()).then(chats))
        .or(warp::path!("chats" / i64)
            .and(warp::get())
            .then(chats_from))
        .or(warp::path!("chat" / String).and(warp::get()).then(chat))
        .or(warp::path!("chat_from" / String / i64)
            .and(warp::get())
            .then(chat_from))
        .or(warp::path("i").and(warp::get()).then(next_id))
        .or(warp::path("ip")
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::json::<ByIP>())
            .and_then(handle_by_ip))
        .with(cors);

    Ok(routes.boxed().into())
}

/*macro_rules! qdrant_path {
    ($fmt:expr, $($arg:tt)*) => {
        format!("{}/{}", env::var("QDRANT_URL")?, format!($fmt, $($arg)*))
    };
} */

// --- HANDLERS ---

async fn i_handler() -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();
    Ok(warp::reply::with_status(
        next_i(&client).await?.to_string(),
        warp::http::StatusCode::OK,
    ))
}

async fn next_i(client: &reqwest::Client) -> AppResult<i64> {
    let res: serde_json::Value = client
        .get(qdrant_path(&format!("collections/{}/points/1", COLLECTION)).await?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .send()
        .await
        .map_err(|e| AppError::new("get_point request", e))?
        .json()
        .await
        .map_err(|e| AppError::new("parse get_point response", e))?;

    let initial_value: i64 = res["result"]["payload"]["u"]
        .as_i64()
        .ok_or(AppError::new_plain("u not found or not an integer"))?;

    let mut body = serde_json::Map::new();
    body.insert("payload".to_string(), json!({ "u": initial_value + 1 }));
    body.insert("points".to_string(), json!("i"));

    client
        .post(qdrant_path(&format!("collections/{}/points/payload", COLLECTION)).await?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::new("update_point request", e))?;

    Ok(initial_value)
}

async fn handle_search(q: SearchQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();
    let mut body = serde_json::Map::new();
    body.insert(
        "vector".to_string(),
        embedding(&q.q)
            .await
            .map_err(|e| AppError::new("q to string in handle_search", e))?
            .into(),
    );
    body.insert("limit".to_string(), 7.into());
    body.insert("with_payload".to_string(), json!(["m", "u"]));
    if let Some(f) = q.f {
        println!("saw f: {:#?}", f);
        let mut must = vec![];
        for key in f.keys() {
            if let Some(v) = f.get(key) {
                must.push(json!({"key": key, "match": {"value": v}}))
            }
        }
        println!("must: {}", serde_json::to_string(&must).unwrap());
        body.insert("filter".to_string(), json!({"must": must}));
    }
    // println!("body: {}", serde_json::to_string(&body).unwrap());
    let res: serde_json::Value = client
        .post(qdrant_path(&format!("collections/{}/points/search", COLLECTION)).await?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .json(&body)
        .send()
        .await
        .map_err(|e| warp::reject::custom(AppError::new("search_points request", e)))?
        .json()
        .await
        .map_err(|e| warp::reject::custom(AppError::new("parse search_points response", e)))?;

    Ok(warp::reply::json(&res["result"]))
    // println!("{:#?}", res);
    // Ok(warp::reply::with_status("dir", warp::http::StatusCode::OK))
}

async fn handle_by_ip(q: ByIP) -> Result<impl warp::Reply, warp::Rejection> {
    let res: serde_json::Value = reqwest::Client::new()
        .post(qdrant_path("collections/i/points/query/groups").await?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .json(&json!({
          "group_by": "ip",
          "limit": 7,
          "group_size": 1,
          "order_by": [
            {
              "key": "d",
              "order": "asc"
            }
          ],
          "with_payload": [
            "ip",
          ],
          "filter": {
            "must": [
              {
                "key": "u",
                "match": {
                  "value": 1
                }
              },
              {
                "key": "d",
                "range": {
                  "gte": q.d,
                }
              }
            ]
          }
        }))
        .send()
        .await
        .map_err(|e| warp::reject::custom(AppError::new("search_points request", e)))?
        .json()
        .await
        .map_err(|e| warp::reject::custom(AppError::new("parse search_points response", e)))?;

    Ok(warp::reply::json(&res["result"]))
}

async fn handle_group_search(q: GroupSearch) -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();
    let mut body = serde_json::Map::new();
    body.insert(
        "vector".to_string(),
        embedding(&q.q)
            .await
            .map_err(|e| AppError::new("q to string in handle_search", e))?
            .into(),
    );
    body.insert("group_by".to_string(), q.k.into());
    body.insert("limit".to_string(), 7.into());
    body.insert("group_size".to_string(), 1.into());
    body.insert("with_payload".to_string(), json!(["m", "u"]));
    if let Some(f) = q.f {
        println!("saw f: {:#?}", f);
        let mut must = vec![];
        for key in f.keys() {
            if let Some(v) = f.get(key) {
                must.push(json!({"key": key, "match": {"value": v}}))
            }
        }
        println!("must: {}", serde_json::to_string(&must).unwrap());
        body.insert("filter".to_string(), json!({"must": must}));
    }
    // println!("body: {}", serde_json::to_string(&body).unwrap());
    let res: serde_json::Value = client
        .post(qdrant_path(&format!("collections/{}/points/search/group", COLLECTION)).await?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .json(&body)
        .send()
        .await
        .map_err(|e| warp::reject::custom(AppError::new("search_points request", e)))?
        .json()
        .await
        .map_err(|e| warp::reject::custom(AppError::new("parse search_points response", e)))?;

    Ok(warp::reply::json(&res["result"]))
    // println!("{:#?}", res);
    // Ok(warp::reply::with_status("dir", warp::http::StatusCode::OK))
}

async fn handle_get(query: ItemQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();

    match get_point_payload(&client, &query.i).await {
        Ok(payload) => {
            if PRIVATE.contains(&payload.c.as_str()) {
                if payload.u == query.u.as_str() {
                    let response_payload = payload.clone();
                    Ok(warp::reply::with_status(
                        warp::reply::json(&response_payload.v),
                        warp::http::StatusCode::OK,
                    ))
                } else {
                    Ok(warp::reply::with_status(
                        warp::reply::json(&"Unauthorized".to_string()),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ))
                }
            } else {
                Ok(warp::reply::with_status(
                    warp::reply::json(&payload),
                    warp::http::StatusCode::OK,
                ))
            }
        }
        Err(_) => Ok(warp::reply::with_status(
            warp::reply::json(&"Not Found".to_string()),
            warp::http::StatusCode::NOT_FOUND,
        )),
    }
}

async fn handle_delete(query: ItemQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();
    let item = get_point_payload(&client, &query.i)
        .await
        .map_err(warp::reject::custom)?;

    if item.u == query.u {
        client
            .post(qdrant_path(&format!("/collections/{}/points/delete", COLLECTION)).await?)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"
            {{
                "points": [{}]
            }}
            "#,
                query.i
            ))
            .send()
            .await
            .map_err(|e| warp::reject::custom(AppError::new("delete_point request", e)))?;

        Ok(warp::reply::with_status(
            "Deleted",
            warp::http::StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            "Unauthorized",
            warp::http::StatusCode::UNAUTHORIZED,
        ))
    }
}

// todo embed chat function

async fn handle_set(s: Set) -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();
    match get_point_payload(&client, &s.i).await {
        Ok(_existing_item) => {
            // if existing_item.u == s.u {
            set(&client, s).await.map_err(warp::reject::custom)?;
            Ok(warp::reply::with_status(
                "Updated",
                warp::http::StatusCode::OK,
            ))
            // } else {
            //     Ok(warp::reply::with_status(
            //         "Unauthorized",
            //         warp::http::StatusCode::UNAUTHORIZED,
            //     ))
            // }
        }
        Err(_) => {
            set(&client, s).await.map_err(warp::reject::custom)?;
            Ok(warp::reply::with_status(
                "Inserted",
                warp::http::StatusCode::CREATED,
            ))
        }
    }
}

async fn get_point_payload(client: &reqwest::Client, i: &str) -> AppResult<Payload> {
    let response: Response = client
        .get(qdrant_path(&format!("/collections/{}/points/{}", COLLECTION, i)).await?)
        .send()
        .await
        .map_err(|e| AppError::new("get_point request", e))?
        .json()
        .await
        .map_err(|e| AppError::new("parse get_point response", e))?;

    Ok(response.result[0]
        .payload
        .clone()
        .ok_or(AppError::new_plain(
            "get_point_payload - no payload on point",
        ))?)
}

async fn set(client: &reqwest::Client, s: Set) -> AppResult<()> {
    client
        .put(qdrant_path(&format!("collections/{}/points?wait", COLLECTION)).await?)
        .body(format!(
            r#"{{points: [{{"id":"{}", "payload": {}, "vector": {}, }}]}}"#,
            s.i,
            s.v,
            embedding(&s.v).await?.to_string()
        ))
        .send()
        .await
        .map_err(|e| AppError::new("upsert_points", e))?;
    Ok(())
}

#[derive(Deserialize)]
// #[serde(untagged)]
struct SearchQuery {
    q: String, // Query string
    f: Option<std::collections::HashMap<String, serde_json::Value>>, // l: Option<u64>,         // Limit
                                                                     // r: Option<Vec<String>>, // Attributes to return
}

#[derive(Deserialize, Serialize, Clone)]
struct Item {
    u: String,            // User
    i: String,            // ID
    v: serde_json::Value, // Value field
    p: bool,              // Private field
}

#[derive(Deserialize)]
struct ItemQuery {
    u: String,
    i: String,
    c: String,
}

#[derive(Debug, Deserialize)]
struct Set {
    // u: String, // user
    i: String,
    v: String, // value
               // p: bool // private
}
#[derive(Serialize, Deserialize, Clone)]
struct Payload {
    c: String, //category the point belongs to
    u: String, //user that created it
    v: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SN {
    String(String),
    Integer(i64),
}
#[derive(Deserialize)]
struct ResponseResult {
    id: Option<SN>,
    version: i64,
    score: f32,
    payload: Option<Payload>,
    vector: Option<serde_json::Value>,
    shard_key: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct Response {
    time: Option<f32>,
    status: Option<String>,
    result: Vec<ResponseResult>,
}

#[derive(Deserialize)]
struct GroupSearch {
    k: String,
    q: String,
    f: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct ByIP {
    d: i64,
    p: i64,
}
