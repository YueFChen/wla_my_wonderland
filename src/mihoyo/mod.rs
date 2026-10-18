//! 米哈游账号、角色和创作者资料 API 客户端。

mod client;
mod creator;
mod endpoint;
mod role;

pub use client::MihoyoClient;
pub use endpoint::API_MICREATOR;

pub use client::cookie_header_for;
