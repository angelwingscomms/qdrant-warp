use serde_json::json;
use warp::reply::Reply;

use crate::{
    app::AppResult,
    qdrant::{qdrant_path, qdrant_put},
    util::{embedding, id},
};

#[derive(serde::Deserialize)]
pub struct Add {
    a: String,
    u: String,
    ad: String,
    ud: String,
    i: String,
    p: String,
}

pub async fn add(s: Add, addr: Option<std::net::SocketAddr>) -> impl Reply {
    f(s, addr).await.map_or_else(
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

async fn f(s: Add, addr: Option<std::net::SocketAddr>) -> AppResult<String> {
    let id = id().await?;
    if let Some(a) = addr {
        qdrant_put(
            &qdrant_path("collections/i/points").await?,
            json!({"points":[{"id":id, "payload": {"u": 1, "a": a.ip().to_string(), "m": s.u, "c": "m", "i": s.i, "p": s.p, "d": s.ud}, "vector": embedding(&s.u).await? }]}),
        ).await?;
    } else {
        qdrant_put(
            &qdrant_path("collections/i/points").await?,
            json!({"points":[{"id":id, "payload": {"u": 1, "m": s.u, "c": "m", "i": s.i, "p": s.p, "d": s.ud}, "vector": embedding(&s.u).await? }]}),
        ).await?;
    }
    qdrant_put(
        &qdrant_path("collections/i/points").await?,
        json!({"points":[{"id":id, "payload": {"u": 0, "m": s.a, "c": "m", "i": s.i, "p": s.p, "d": s.ad}, "vector": embedding(&s.a).await? }]}),
    ).await?;
    Ok(id)
}
