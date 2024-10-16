use serde_json::json;
use crate::app::AppError;
use crate::constants::{I_ID, SECRETS};

use crate::{app::AppResult, qdrant::{qdrant_path, qdrant_post}};

pub fn random_embedding() -> Vec<f64> {
    vec![]
}

pub async fn id() -> AppResult<String> {
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

pub async fn embedding(query: &str) -> AppResult<serde_json::Value> {
    let url = SECRETS
        .lock()
        .await
        .get("EMBEDDING_URL")
        .ok_or("QDRANT_KEY not found in env")
        .map_err(|e| AppError::new_plain(e))?;
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