use serde_json::json;
use warp::reply::Reply;

use crate::{
    constants::AppResult,
    qdrant::{qdrant_path, qdrant_post},
};

pub async fn handle_scroll_chats(page: i64) -> impl Reply {
    scroll_chats(page).await.map_or_else(
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

pub async fn scroll_chats(page: i64) -> AppResult<serde_json::Value> {
    Ok(qdrant_post(
        &qdrant_path("collections/i/points/scroll").await?,
        json!({"offset": (page - 1) * 7,  "limit": 7}),
    )
    .await?["points"]
        .clone())
}
