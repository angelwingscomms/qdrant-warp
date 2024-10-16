use serde_json::json;
use warp::reply::Reply;

use crate::{
    constants::AppResult,
    qdrant::{qdrant_path, qdrant_post},
};

pub async fn chat_from(id: String, from: String) -> impl Reply {
    f(id, from).await.map_or_else(
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

pub async fn f(id: String, from: String) -> AppResult<String> {
    Ok(qdrant_post(
        &qdrant_path("collections/i/points/scroll").await?,
        json!({"limit": 7, "order_by": {"key": "d", "direction": "desc", "start_from": from}, "filter": {"must_not": {"key": "d", }, "must": [{"key": "c", "match": {"value": "scm"}}, {"key": "i", "match": {"value": id}}]}}),
    )
    .await?["result"]["points"]
        .to_string())
}
