//! Provider-specific login orchestration belongs to the plugin.
use crate::app::account_error;
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use wonderland_plugin_sdk::{HostClient, PluginError, RequestTracker};
use wonderland_wla_my_wonderland::Platform;
use wonderland_wla_my_wonderland::account::{
    AccountError, AccountService, FsAccountService, LoginCancelReason, LoginOutcome, StoredCookie,
    evaluate, select_cookies,
};
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
        platform: Platform,
        id: &str,
    ) -> Result<Value, PluginError> {
        let _active = self
            .active
            .begin(id)
            .ok_or_else(|| PluginError::new("BUSY", "已有登录正在进行"))?;
        self.cancelled.store(false, Ordering::Release);
        account.begin_login(platform).map_err(account_error)?;
        let _ = host.emit("account.changed", None, json!({}));
        let opened=host.call_core("core.browser.session.open",json!({"url":platform.login_url(),"title":platform.login_title(),"profile":platform.browser_profile(),"reset":true,"width":1000,"height":760}));
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
                let cookies = select_cookies(platform, &cookies);
                if settle_until.is_none() && evaluate(platform, &cookies).is_some() {
                    settle_until = Some(Instant::now() + Duration::from_secs(2));
                } else if settle_until.is_some_and(|until| Instant::now() >= until) {
                    match runtime.block_on(account.validate_login(platform, &cookies)) {
                        Ok(()) => {}
                        Err(AccountError::SessionExpired | AccountError::NotLoggedIn) => {
                            // The account SDK may still be propagating cookies. Keep the login page usable.
                            settle_until = None;
                            thread::sleep(Duration::from_millis(600));
                            continue;
                        }
                        Err(error) => return Err(account_error(error)),
                    }
                    if self.cancelled.load(Ordering::Acquire) {
                        account.cancel_login(LoginCancelReason::Cancelled);
                        return Ok(account.snapshot());
                    }
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
