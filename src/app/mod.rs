use thiserror::Error;
pub type AppResult<T> = Result<T, AppError>;

#[derive(Error, Debug)]
pub struct AppError {
    t: String,
}

impl warp::reject::Reject for AppError {}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.t)
    }
}
impl AppError {
    pub fn new(m: &str, e: impl std::error::Error) -> Self {
        AppError {
            t: format!("{}: {}", m, e.to_string()),
        }
    }

    pub fn new_plain(m: &str) -> Self {
        AppError { t: m.to_string() }
    }
}