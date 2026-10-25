//! 账号凭据验证、加密存储和账号状态管理。

pub mod cookie;
mod service;
mod store;

pub use cookie::{Credentials, evaluate, select_cookies};
pub use service::FsAccountService;

pub mod migration;
mod model;
pub use crate::account_error::AccountError;
pub use model::*;

#[cfg(all(test, windows))]
mod tests;
