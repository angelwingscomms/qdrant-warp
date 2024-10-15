use serde::Serialize;

use crate::{app::{AppResult, AppError}, constants::SECRETS};

pub async fn qdrant_path(path: &str) -> AppResult<String> {
    Ok(format!(
        "{}/{}",
        SECRETS
            .lock()
            .await
            .get("QDRANT_URL")
            .ok_or("QDRANT_KEY not found in env")
            .map_err(|e| AppError::new_plain(e))?,
        path
    ))
}

pub async fn qdrant_put(path: &str, body: impl Serialize) -> AppResult<serde_json::Value> {
    Ok(reqwest::Client::new()
        .put(path)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::new("upsert_points", e))?
        .json()
        .await
        .map_err(|e| AppError::new("qdrant response to json", e))?)
}

pub async fn qdrant_post(path: &str, body: impl Serialize) -> AppResult<serde_json::Value> {
    Ok(reqwest::Client::new()
        .post(path)
        .header(
            "api-key",
            SECRETS
                .lock()
                .await
                .get("QDRANT_KEY")
                .ok_or("QDRANT_KEY not found in env")
                .map_err(|e| AppError::new_plain(e))?,
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::new("upsert_points", e))?
        .json()
        .await
        .map_err(|e| AppError::new("qdrant response to json", e))?)
}