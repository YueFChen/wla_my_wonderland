//! Mihoyo login orchestration belongs to this plugin, never the host window adapter.
use crate::app::account_error;
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use wonderland_my_wonderland::account::{
    AccountService, FsAccountService, LoginCancelReason, LoginOutcome, StoredCookie, evaluate,
    select_mihoyo_cookies,
};
use wonderland_plugin_sdk::{HostClient, PluginError, RequestTracker};
const LOGIN_URL: &str = "https://act.mihoyo.com/miliastra_wonderland/developer";
const SETTLE_URL: &str = "https://api-micreator.mihoyo.com";
#[derive(Default)]
pub struct Login {
    cancelled: AtomicBool,
    active: RequestTracker,
}
impl Login {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release)
    }
    pub fn cancel_request(&self, id: &str) {
        if self.active.matches(id) {
            self.cancel()
        }
    }
    pub fn run(
        &self,
        host: &HostClient,
        account: &FsAccountService,
        runtime: &tokio::runtime::Runtime,
        id: &str,
    ) -> Result<Value, PluginError> {
        let _active = self
            .active
            .begin(id)
            .ok_or_else(|| PluginError::new("BUSY", "已有登录正在进行"))?;
        self.cancelled.store(false, Ordering::Release);
        account.begin_login().map_err(account_error)?;
        let _ = host.emit("account.changed", None, json!({}));
        let opened=host.call_core("core.browser.session.open",json!({"url":LOGIN_URL,"title":"登录米哈游通行证","profile":"mihoyo-login","reset":true,"width":1000,"height":760}));
        let opened = match opened {
            Ok(value) => value,
            Err(error) => {
                account.cancel_login(LoginCancelReason::Failed);
                let _ = host.emit("account.changed", None, json!({}));
                return Err(error);
            }
        };
        let Some(session) = opened.get("sessionId").and_then(Value::as_str) else {
            account.cancel_login(LoginCancelReason::Failed);
            let _ = host.emit("account.changed", None, json!({}));
            return Err(PluginError::new(
                "INVALID_RESPONSE",
                "Host did not return a browser session.",
            ));
        };
        let result = (|| {
            let mut settle_until = None;
            loop {
                if self.cancelled.load(Ordering::Acquire) {
                    account.cancel_login(LoginCancelReason::Cancelled);
                    return Ok(account.snapshot());
                }
                let value = match host
                    .call_core("core.browser.session.cookies", json!({"sessionId":session}))
                {
                    Ok(value) => value,
                    Err(error) => {
                        account.cancel_login(if error.code == "BROWSER_SESSION_CLOSED" {
                            LoginCancelReason::WindowClosed
                        } else if error.code == "CANCELLED" {
                            LoginCancelReason::Cancelled
                        } else {
                            LoginCancelReason::Failed
                        });
                        return Err(error);
                    }
                };
                let cookies: Vec<StoredCookie> = serde_json::from_value(value).map_err(|_| {
                    PluginError::new("INVALID_RESPONSE", "Invalid browser cookie snapshot.")
                })?;
                let cookies = select_mihoyo_cookies(&cookies);
                if settle_until.is_none() && evaluate(&cookies).is_some() {
                    host.call_core(
                        "core.browser.session.visibility",
                        json!({"sessionId":session,"visible":false}),
                    )?;
                    host.call_core(
                        "core.browser.session.navigate",
                        json!({"sessionId":session,"url":SETTLE_URL}),
                    )?;
                    settle_until = Some(Instant::now() + Duration::from_secs(2));
                } else if settle_until.is_some_and(|until| Instant::now() >= until) {
                    match account.submit_login(cookies).map_err(account_error)? {
                        LoginOutcome::Completed { account_key } => {
                            if let Err(error) =
                                runtime.block_on(account.sync_account_info(&account_key))
                            {
                                tracing::warn!(code = error.code(), "账号已保存，资料同步暂未完成");
                            }
                            return Ok(account.snapshot());
                        }
                        LoginOutcome::Pending => settle_until = None,
                    }
                }
                thread::sleep(Duration::from_millis(600));
            }
        })();
        // Core also closes this resource when the invocation is cancelled or the process exits.
        let _ = host.call_core("core.browser.session.close", json!({"sessionId":session}));
        if result.is_err() && account.snapshot().status.is_logging_in() {
            account.cancel_login(LoginCancelReason::Failed)
        }
        let _ = host.emit("account.changed", None, json!({}));
        serde_json::to_value(result?)
            .map_err(|_| PluginError::new("INVALID_RESPONSE", "Cannot serialize account data."))
    }
}
