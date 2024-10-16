use serde_json::json;
use warp::reply::Reply;

use crate::{
    constants::AppResult,
    qdrant::{qdrant_path, qdrant_post},
};

pub async fn chats_from(from: String) -> impl Reply {
    f(from).await.map_or_else(
        |e| {
            log::error!("{:#?}", e);
            warp::reply::with_status(
                "An error occured on our side".to_string(),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            )
        },
        |v| warp::reply::with_status(v, warp::http::StatusCode::OK),
    )
}

pub async fn f(from: String) -> AppResult<String> {
    let res = qdrant_post(
        &qdrant_path("collections/i/points/scroll").await?,
        json!({"limit": 7, "order_by": {"key": "d", "direction": "desc", "start_from": from}, "filter": {"must": [{"key": "c", "match": {"value": "lucid"}}]}}),
    )
    .await?;
    println!("chats res: {}", res);
    Ok(res["result"]["points"].to_string())
}
