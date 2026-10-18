use crate::login::Login;
use serde_json::{Value, json};
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};
use wonderland_my_wonderland::{
    AccountProvider, AccountSnapshot, MyWonderland, Scope,
    account::{AccountError, AccountService, FsAccountService, migration},
    platform_http::HostScope,
};
use wonderland_plugin_sdk::{HostClient, PluginError, PluginFailure, serve};
const CONTRACT: &str = include_str!("../package/contract.json");
struct Accounts(Arc<FsAccountService>);
impl AccountProvider for Accounts {
    fn snapshot(&self) -> AccountSnapshot {
        self.0.snapshot()
    }
    fn authed_get<'a>(
        &'a self,
        key: &'a str,
        path: &'a str,
        query: &'a [(String, String)],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PluginFailure>> + Send + 'a>> {
        Box::pin(async move {
            self.0
                .authed_get(key, path, query)
                .await
                .map_err(business_error)
        })
    }
}
fn business_error(error: AccountError) -> PluginFailure {
    match error {
        AccountError::NotLoggedIn => PluginFailure::NotLoggedIn,
        AccountError::SessionExpired => PluginFailure::SessionExpired,
        AccountError::AccountNotFound(key) => PluginFailure::AccountNotFound(key),
        AccountError::Timeout => PluginFailure::Timeout,
        AccountError::Connection => PluginFailure::Connection,
        AccountError::Http(code) => PluginFailure::Http(code),
        AccountError::InvalidInput => PluginFailure::InvalidInput,
        AccountError::InvalidResponse => PluginFailure::InvalidResponse,
        _ => PluginFailure::Other(error.to_string()),
    }
}
pub(crate) fn account_error(error: AccountError) -> PluginError {
    PluginError::new(
        format!("PLUGIN_MY_WONDERLAND_{}", error.code().to_ascii_uppercase()),
        error.to_string(),
    )
}
fn changed(host: &HostClient) {
    let _ = host.emit("account.changed", None, json!({}));
}
pub fn run() -> Result<(), String> {
    let data = std::env::var_os("WONDERLAND_PLUGIN_DATA_DIR")
        .map(PathBuf::from)
        .ok_or("Core did not provide a plugin data directory")?;
    let legacy = std::env::var_os("WONDERLAND_HOST_DATA_DIR").map(PathBuf::from);
    migration::import_legacy(&data, legacy.as_deref()).map_err(|e| e.to_string())?;
    let account = Arc::new(FsAccountService::load(data.clone()).map_err(|e| e.to_string())?);
    let plugin = Arc::new(
        MyWonderland::new(data.join("series-v2"), Arc::new(Accounts(account.clone())))
            .map_err(|e| e.to_string())?,
    );
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?,
    );
    let login = Arc::new(Login::default());
    serve(
        "my_wonderland",
        env!("CARGO_PKG_VERSION"),
        CONTRACT,
        move |host, method, params, request_id| {
            let _scope = HostScope::enter(host.clone());
            match method.as_str() {
                "__cancel" => {
                    if let Some(id) = params.get("requestId").and_then(Value::as_str) {
                        login.cancel_request(id)
                    }
                    Ok(Value::Null)
                }
                "account_snapshot" => serde_json::to_value(account.snapshot()).map_err(|_| {
                    PluginError::new("INVALID_RESPONSE", "Cannot serialize account data.")
                }),
                "account_login" => login.run(
                    &host,
                    &account,
                    &runtime,
                    request_id.as_deref().unwrap_or(""),
                ),
                "account_login_cancel" => {
                    login.cancel();
                    Ok(json!({"ok":true}))
                }
                "account_sync" => {
                    let key = key(&params)?;
                    let result = runtime
                        .block_on(account.sync_account_info(key))
                        .map_err(account_error)?;
                    changed(&host);
                    Ok(serde_json::to_value(result).unwrap())
                }
                "account_switch" | "account_remove" => {
                    if matches!(
                        account.snapshot().status,
                        wonderland_my_wonderland::AccountStatus::LoggingIn
                    ) {
                        return Err(PluginError::new("BUSY", "请先结束当前登录"));
                    }
                    let key = key(&params)?;
                    if method == "account_switch" {
                        account.switch_account(key)
                    } else {
                        account.remove_account(key)
                    }
                    .map_err(account_error)?;
                    changed(&host);
                    Ok(json!({"ok":true}))
                }
                "series" | "collect" => {
                    let scope: Scope = serde_json::from_value(
                        params
                            .get("scope")
                            .cloned()
                            .ok_or_else(|| PluginError::new("INVALID_INPUT", "Missing scope."))?,
                    )
                    .map_err(|_| PluginError::new("INVALID_INPUT", "Invalid scope."))?;
                    let value = if method == "series" {
                        serde_json::to_value(plugin.series(&scope).map_err(|e| {
                            PluginError::new(
                                format!("PLUGIN_MY_WONDERLAND_{}", e.code().to_uppercase()),
                                e.to_string(),
                            )
                        })?)
                    } else {
                        serde_json::to_value(runtime.block_on(plugin.collect(scope)).map_err(
                            |e| {
                                PluginError::new(
                                    format!("PLUGIN_MY_WONDERLAND_{}", e.code().to_uppercase()),
                                    e.to_string(),
                                )
                            },
                        )?)
                    };
                    value.map_err(|_| {
                        PluginError::new("INVALID_RESPONSE", "Cannot serialize collection data.")
                    })
                }
                _ => Err(PluginError::new(
                    "METHOD_NOT_FOUND",
                    "Unknown my_wonderland method.",
                )),
            }
        },
    )
}
fn key(params: &Value) -> Result<&str, PluginError> {
    params
        .get("account_key")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PluginError::new("INVALID_INPUT", "Missing account key."))
}
