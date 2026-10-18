//! Plugin platform requests use the Core's ordinary HTTP transport and proxy.
use crate::account::AccountError;
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use wonderland_plugin_sdk::{HostClient, HttpHeader, HttpRequestParams, PluginError};

thread_local! {static HOST:RefCell<Option<HostClient>>=const{RefCell::new(None)};}
pub struct HostScope(Option<HostClient>);
impl HostScope {
    pub fn enter(host: HostClient) -> Self {
        Self(HOST.with(|slot| slot.replace(Some(host))))
    }
}
impl Drop for HostScope {
    fn drop(&mut self) {
        HOST.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}
pub fn current_host() -> Result<HostClient, AccountError> {
    HOST.with(|slot| slot.borrow().clone())
        .ok_or(AccountError::NotInitialized)
}
pub fn host_error(error: PluginError) -> AccountError {
    match error.code.as_str() {
        "TIMEOUT" => AccountError::Timeout,
        "CANCELLED" => AccountError::Other("操作已取消".into()),
        "BROWSER_SESSION_CLOSED" => AccountError::LoginWindow("登录窗口已关闭".into()),
        "INTERACTION_UNAVAILABLE" => {
            AccountError::LoginWindow("当前环境无法交互，请在宿主桌面登录".into())
        }
        _ => AccountError::Transport(error.message),
    }
}
pub struct HttpClient;
impl HttpClient {
    pub fn new() -> Result<Self, AccountError> {
        Ok(Self)
    }
    pub async fn get_bytes(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        query: &[(String, String)],
    ) -> Result<Vec<u8>, AccountError> {
        let host = current_host()?;
        let mut request: HttpRequestParams = serde_json::from_value(
            serde_json::json!({"method":"GET","url":url,"headers":[],"query":query}),
        )
        .map_err(|_| AccountError::InvalidInput)?;
        request.headers = headers
            .iter()
            .map(|(name, value)| HttpHeader {
                name: (*name).into(),
                value: (*value).into(),
            })
            .collect();
        request.headers.push(HttpHeader {
            name: "user-agent".into(),
            value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)".into(),
        });
        let (meta, bytes) = host.http_request_bytes(&request).map_err(host_error)?;
        if !(200..300).contains(&meta.status) {
            return Err(AccountError::Http(meta.status));
        }
        Ok(bytes)
    }
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<T, AccountError> {
        serde_json::from_slice(&self.get_bytes(url, headers, &[]).await?)
            .map_err(|_| AccountError::InvalidResponse)
    }
}
