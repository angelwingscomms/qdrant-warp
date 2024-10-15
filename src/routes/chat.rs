use serde_json::json;
use warp::reply::Reply;

use crate::{
    constants::AppResult,
    qdrant::{qdrant_path, qdrant_post},
};

pub async fn handle_chat(id: String, page: i64) -> impl Reply {
    chat(id, page).await.map_or_else(
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

pub async fn chat(id: String, page: i64) -> AppResult<String> {
    Ok(qdrant_post(
        &qdrant_path("collections/i/points/scroll").await?,
        json!({"offset": (page - 1) * 7,  "limit": 7, "order_by": {"key": "d", "direction": "desc"}, "filter": {"must": [{"key": "c", "match": {"value": "scm"}}, {"key": "i", "match": {"value": id}}]}}),
    )
    .await?["points"]
        .to_string())
}
