use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use base64::Engine as _;
use serde_json::{Value, json};
use wonderland_my_wonderland::{AccountProvider, MyWonderland, Scope};
use wonderland_plugin_sdk::{
    AccountSnapshot, AccountStatus, HostClient, PluginError, PluginFailure, serve,
};

const CONTRACT: &str = include_str!("../package/contract.json");

thread_local! {
    static INVOCATION_ACCOUNT: std::cell::RefCell<Option<(HostClient, Option<AccountSnapshot>)>> = const { std::cell::RefCell::new(None) };
}

struct InvocationAccountGuard(Option<(HostClient, Option<AccountSnapshot>)>);

impl InvocationAccountGuard {
    fn new(host: HostClient) -> Self {
        Self(INVOCATION_ACCOUNT.with(|current| current.replace(Some((host, None)))))
    }
}

impl Drop for InvocationAccountGuard {
    fn drop(&mut self) {
        INVOCATION_ACCOUNT.with(|current| current.replace(self.0.take()));
    }
}

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
    if let Err(error) = run() {
        eprintln!("my_wonderland backend stopped: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let data_dir = std::env::var_os("WONDERLAND_PLUGIN_DATA_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "Core did not provide a plugin data directory".to_owned())?;
    let account = Arc::new(HostAccountService::new());
    let plugin = Arc::new(
        MyWonderland::new(data_dir.join("series-v2"), account.clone())
            .map_err(|error| error.to_string())?,
    );
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?,
    );

    serve(
        "my_wonderland",
        env!("CARGO_PKG_VERSION"),
        CONTRACT,
        move |host, method, params, _request_id| {
            let _scope = InvocationAccountGuard::new(host.clone());
            let result = dispatch(&host, &account, &plugin, &runtime, &method, params);
            result.map_err(|error| {
                let code = format!("PLUGIN_MY_WONDERLAND_{}", error.code().to_ascii_uppercase());
                PluginError::new(code, error.to_string())
            })
        },
    )
    .map_err(|error| error.to_string())
}

fn dispatch(
    host: &HostClient,
    account: &Arc<HostAccountService>,
    plugin: &MyWonderland,
    runtime: &tokio::runtime::Runtime,
    method: &str,
    params: Value,
) -> Result<Value, PluginFailure> {
    match method {
        "account_snapshot" => {
            account.refresh(host)?;
            serde_json::to_value(account.snapshot()).map_err(|_| PluginFailure::InvalidResponse)
        }
        "series" => {
            account.refresh(host)?;
            let scope: Scope = value_param(&params, "scope")?;
            serde_json::to_value(plugin.series(&scope)?).map_err(|_| PluginFailure::InvalidResponse)
        }
        "collect" => {
            account.refresh(host)?;
            let scope: Scope = value_param(&params, "scope")?;
            let series = runtime.block_on(plugin.collect(scope))?;
            serde_json::to_value(series).map_err(|_| PluginFailure::InvalidResponse)
        }
        _ => Err(PluginFailure::InvalidInput),
    }
}

fn value_param<T: serde::de::DeserializeOwned>(
    params: &Value,
    key: &str,
) -> Result<T, PluginFailure> {
    serde_json::from_value(
        params
            .get(key)
            .cloned()
            .ok_or(PluginFailure::InvalidInput)?,
    )
    .map_err(|_| PluginFailure::InvalidInput)
}

struct HostAccountService {
    snapshot: RwLock<AccountSnapshot>,
}

impl HostAccountService {
    fn new() -> Self {
        Self {
            snapshot: RwLock::new(AccountSnapshot {
                status: AccountStatus::LoggedOut,
                current_account_key: None,
                accounts: Vec::new(),
                last_login_failure: None,
            }),
        }
    }

    fn refresh(&self, host: &HostClient) -> Result<(), PluginFailure> {
        let value = host
            .call_core("core.account.snapshot", json!({}))
            .map_err(host_error)?;
        let snapshot: AccountSnapshot =
            serde_json::from_value(value).map_err(|_| PluginFailure::InvalidResponse)?;
        INVOCATION_ACCOUNT.with(|current| {
            if let Some((_, invocation_snapshot)) = current.borrow_mut().as_mut() {
                *invocation_snapshot = Some(snapshot.clone());
            }
        });
        *self
            .snapshot
            .write()
            .map_err(|_| PluginFailure::NotInitialized)? = snapshot;
        Ok(())
    }
}

impl AccountProvider for HostAccountService {
    fn authed_get<'a>(
        &'a self,
        account_key: &'a str,
        path: &'a str,
        query: &'a [(String, String)],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PluginFailure>> + Send + 'a>> {
        // The service call is synchronous over the SDK's multiplexed stdio channel. Wrapping it
        // in the kernel's async trait keeps the legacy business algorithm independent of Tauri.
        // The collect future is driven by Runtime::block_on on its request thread.
        // Capture that request's host before constructing the async account call;
        // another device's polling must not change its context or cancellation.
        let host = INVOCATION_ACCOUNT
            .with(|current| current.borrow().as_ref().map(|(host, _)| host.clone()));
        Box::pin(async move {
            let host = host.ok_or(PluginFailure::NotInitialized)?;
            let response = host
                .call_core(
                    "core.account.authed_get",
                    json!({
                        "accountKey": account_key,
                        "path": path,
                        "query": query,
                    }),
                )
                .map_err(host_error)?;
            let encoded = response
                .get("contentBase64")
                .and_then(Value::as_str)
                .ok_or(PluginFailure::InvalidResponse)?;
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|_| PluginFailure::InvalidResponse)
        })
    }

    fn snapshot(&self) -> AccountSnapshot {
        if let Some(snapshot) = INVOCATION_ACCOUNT.with(|current| {
            current
                .borrow()
                .as_ref()
                .and_then(|(_, snapshot)| snapshot.clone())
        }) {
            return snapshot;
        }
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }
}

fn host_error(error: PluginError) -> PluginFailure {
    match error.code.as_str() {
        "TIMEOUT" => PluginFailure::Timeout,
        "UNAUTHORIZED" => PluginFailure::InvalidInput,
        "NOT_LOGGED_IN" => PluginFailure::NotLoggedIn,
        "SESSION_EXPIRED" => PluginFailure::SessionExpired,
        "INVALID_INPUT" => PluginFailure::InvalidInput,
        _ => PluginFailure::Transport(error.message),
    }
}
