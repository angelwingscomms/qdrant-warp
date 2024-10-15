use shuttle_runtime::SecretStore;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;

use crate::app::AppError;

pub type AppResult<T> = Result<T, AppError>;
pub const COLLECTION: &'static str = "i";
pub const SITE_CHAT_MESSAGE_CATEGORY: &'static str = "scm";
pub static SECRETS: Lazy<Mutex<SecretStore>> =
    Lazy::new(|| Mutex::new(SecretStore::new(std::collections::BTreeMap::new())));
pub const PRIVATE: &[&str] = &[""];
