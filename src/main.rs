use anyhow::Result;
use log;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shuttle_runtime::SecretStore;
use std::env;
use thiserror::Error;
use tokio::sync::Mutex;
use warp::{Filter, Reply};

type AppResult<T> = Result<T, AppError>;
const COLLECTION: &'static str = "i";
static SECRETS: Lazy<Mutex<SecretStore>> =
    Lazy::new(|| Mutex::new(SecretStore::new(std::collections::BTreeMap::new())));
const PRIVATE: &[&str] = &[""];

#[derive(Error, Debug)]
struct AppError {
    t: String,
}

impl warp::reject::Reject for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.t)
    }
}
impl AppError {
    fn new(m: &str, e: impl std::error::Error) -> Self {
        AppError {
            t: format!("{}: {}", m, e.to_string()),
        }
    }

    fn new_plain(m: &str) -> Self {
        AppError { t: m.to_string() }
    }
}

#[shuttle_runtime::main]
async fn warp(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_warp::ShuttleWarp<(impl Reply,)> {
    let mut secrets_ = SECRETS.lock().await;
    *secrets_ = secrets;

    // let mut body = serde_json::Map::new();
    // let mut vectors = serde_json::Map::new();
    // vectors.insert("distance".to_string(), "Cosine".into());
    // vectors.insert("size".to_string(), 1024.into());
    // body.insert("vectors".to_string(), vectors.into());

    // let res: serde_json::Value = reqwest::Client::new()
    //     .put(qdrant_path(&format!("collections/{}", COLLECTION)).unwrap())
    //     .header(
    //         "api-key",
    //         SECRETS.lock().await.get("QDRANT_KEY").ok_or("QDRANT_KEY not found in env")
    //             .map_err(|_| AppError::new_plain("get QDRANT_KEY in handle_search"))
    //             .unwrap(),
    //     )
    //     .json(&body)
    //     .send()
    //     .await
    //     .map_err(|e| warp::reject::custom(AppError::new("search_points request", e)))
    //     .unwrap()
    //     .json()
    //     .await
    //     .map_err(|e| warp::reject::custom(AppError::new("parse search_points response", e)))
    //     .unwrap();

    // println!("create collection res: {}", res);

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .allow_headers(vec!["Content-Type"]);

    let get_route = warp::path::end()
        .and(warp::get())
        .and(warp::query::<ItemQuery>())
        .and_then(handle_get);

    let delete_route = warp::path::end()
        .and(warp::delete())
        .and(warp::query::<ItemQuery>())
        .and_then(handle_delete);

    let set_route = warp::path::end()
        .and(warp::put())
        .and(warp::body::json::<Set>())
        .and_then(handle_set);

    let create_route = warp::path::end()
        .and(warp::post())
        .and(warp::body::json::<serde_json::Value>())
        .and(warp::filters::addr::remote())
        .and_then(handle_create);

    let search_route = warp::path("search")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<SearchQuery>())
        .and_then(handle_search);

    let routes = get_route
        // .or(delete_route)
        // .or(set_route)
        .or(create_route)
        .or(search_route)
        .with(cors)
        .recover(handle_error);

    Ok(routes.boxed().into())
}

fn qdrant_path(path: &str) -> AppResult<String> {
    Ok(format!(
        "{}/{}",
        env::var("QDRANT_URL").map_err(|e| AppError::new("QDRANT_URL env var", e))?,
        path
    ))
}

/*macro_rules! qdrant_path {
    ($fmt:expr, $($arg:tt)*) => {
        format!("{}/{}", env::var("QDRANT_URL")?, format!($fmt, $($arg)*))
    };
} */

// --- HANDLERS ---

async fn handle_search(q: SearchQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let client = reqwest::Client::new();
    let mut body = serde_json::Map::new();
    body.insert(
        "vector".to_string(),
        get_embedding(&q.q)
            .await
            .map_err(|e| AppError::new("q to string in handle_search", e))?
            .into(),
    );
    body.insert("limit".to_string(), 7.into());
    body.insert("with_payload".to_string(), json!(["m", "u"]));
    if let Some(f) = q.f {
        let mut must = vec![];
        for key in f.keys() {
            if let Some(v) = f.get(key) {
                must.push(json!({"key": key, "match": {"value": v}}))
            }
        }
        body.insert("filters".into(), json!({"must": must}));
    }
    // println!("body: {}", serde_json::to_string(&body).unwrap());
    let res: serde_json::Value = client
        .post(qdrant_path(&format!(
            "collections/{}/points/search",
            COLLECTION
        ))?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|_| AppError::new_plain("get QDRANT_KEY in handle_search"))?,
        )
        .header("Content-Type", "application/json")
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
            .post(qdrant_path(&format!(
                "/collections/{}/points/delete",
                COLLECTION
            ))?)
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

async fn handle_create(
    mut s: serde_json::Value,
    addr: Option<std::net::SocketAddr>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let id = uuid::Uuid::now_v7().to_string();
    if let Some(a) = addr {
        s["ip"] = json!(a.ip().to_string());
    }
    let s_body =
        serde_json::to_string(&s).map_err(|e| AppError::new("s to string in handle_create", e))?;

    // let mut body = json!({
    //     "points": [{
    //         "id": id,
    //         "payload": s,
    //         "vector":
    //     }]
    // });
    let mut body = serde_json::Map::new();
    let mut point = serde_json::Map::new();
    point.insert("id".into(), id.clone().into());
    point.insert("payload".into(), s.into());
    point.insert("vector".into(), get_embedding(&s_body).await?);
    body.insert("points".into(), json!([point]));

    // let res = 
    reqwest::Client::new()
        .put(qdrant_path(&format!("collections/{}/points", COLLECTION))?)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|_| AppError::new_plain("get QDRANT_KEY in handle_create"))?,
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::new("upsert_points", e))?;
    // println!(
    //     "create res: {:#?}",
    //     res.text()
    //         .await
    //         .map_err(|e| AppError::new("res to json", e))?
    // );
    Ok(warp::reply::with_status(
        id,
        warp::http::StatusCode::CREATED,
    ))
}

async fn handle_error(rejection: warp::Rejection) -> Result<impl Reply, std::convert::Infallible> {
    if let Some(error) = rejection.find::<AppError>() {
        log::error!("{}", error.to_string());
    } else {
        log::error!("{:#?}", rejection);
    }
    Ok(warp::reply::with_status(
        "We had an error",
        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
    ))
}

// --- REQUEST HELPERS ---

async fn reqwest_client() -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", SECRETS.lock().await.get("QDRANT_KEY").ok_or("QDRANT_KEY not found in env")?).parse()?,
    );
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

async fn get_embedding(query: &str) -> AppResult<serde_json::Value> {
    let url = env::var("EMBEDDING_URL")
        .unwrap_or_else(|_| "https://fastembedserver.shuttleapp.rs/embeddings".to_string()); // Comment: This gets the embedding URL from an env var
    Ok(reqwest::Client::new()
        .post(&url)
        .json(&json!({ "input": query }))
        .send()
        .await
        .map_err(|e| AppError::new("sending get_embedding request", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| AppError::new("parsing get_embedding response to json", e))?["data"][0]
        ["embedding"]
        .clone())
}

/*async fn get_embedding_vec(query: &str) -> AppResult<Vec<f32>> {
    let url = env::var("EMBEDDING_URL")
        .unwrap_or_else(|_| "https://fastembedserver.shuttleapp.rs/embeddings".to_string()); // Comment: This gets the embedding URL from an env var
    Ok(reqwest::Client::new()
        .post(&url)
        .json(&json!({ "input": query }))
        .send()
        .await
        .map_err(|e| AppError::new("sending get_embedding request", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| AppError::new("parsing get_embedding response to json", e))?["data"][0]
        ["embedding"]
        .clone()
        .as_array()
        .ok_or_else(|| AppError::new_plain("Failed to get embedding array"))?
        .iter()
        .map(|v| {
            Ok(v.as_f64()
                .ok_or_else(|| AppError::new_plain("Failed to convert embedding value to f64"))?
                as f32)
        })
        .collect::<Result<Vec<f32>, AppError>>()?)
} */

async fn get_point_payload(client: &reqwest::Client, i: &str) -> AppResult<Payload> {
    let response: Response = client
        .get(qdrant_path(&format!(
            "/collections/{}/points/{}",
            COLLECTION, i
        ))?)
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
        .put(qdrant_path(&format!(
            "collections/{}/points?wait",
            COLLECTION
        ))?)
        .body(format!(
            r#"{{points: [{{"id":"{}", "payload": {}, "vector": {}, }}]}}"#,
            s.i,
            s.v,
            get_embedding(&s.v).await?.to_string()
        ))
        .send()
        .await
        .map_err(|e| AppError::new("upsert_points", e))?;
    Ok(())
}

// --- STRUCTS ---

#[derive(Deserialize)]
// #[serde(untagged)]
struct SearchQuery {
    q: String, // Query string
    f: Option<std::collections::HashMap<String, String>>, // l: Option<u64>,         // Limit
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
