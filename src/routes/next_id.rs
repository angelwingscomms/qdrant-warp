use serde_json::json;
use warp::reply::Reply;

use crate::{
    constants::{AppResult, I_ID},
    qdrant::{qdrant_path, qdrant_post},
};

pub async fn next_id() -> impl Reply {
    f().await.map_or_else(
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

pub async fn f() -> AppResult<String> {
    let id: i64 = qdrant_post(
        &qdrant_path("collections/i/points").await?,
        json!({"ids": [I_ID], "with_payload": ["sc"]}),
    )
    .await?["result"][0]["payload"]["sc"]
        .as_i64()
        .unwrap_or(0);
    let next = id + 1;
    qdrant_post(
        &qdrant_path("collections/i/points/payload?wait=true").await?,
        json!({"payload": {"sc": next}, "points": [I_ID]}),
    )
    .await?;
    println!(
        "new_id: {}",
        qdrant_post(
            &qdrant_path("collections/i/points").await?,
            json!({"ids": [I_ID], "with_payload": ["sc"]}),
        )
        .await?
    );
    println!("id, next: {}, {}", id, next);
    Ok(id.to_string())
}
