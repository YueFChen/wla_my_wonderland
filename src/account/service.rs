//! 账号状态管理、凭据持久化与账号资料同步。

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Platform;
use crate::account::{
    Account, AccountError, AccountService, AccountSnapshot, AccountStatus, LoginCancelReason,
    LoginOutcome, StoredCookie,
};
use crate::mihoyo::MihoyoClient;
use tracing::warn;

use super::cookie::{evaluate, select_cookies};
use super::store::CredentialStore;

/// 基于文件系统的账号服务。
pub struct FsAccountService {
    store: CredentialStore,
    mihoyo: MihoyoClient,
    state: Mutex<State>,
}

struct State {
    status: AccountStatus,
    accounts: Vec<Account>,
    current: Option<String>,
    last_login_failure: Option<LoginCancelReason>,
    login_platform: Option<Platform>,
}

impl FsAccountService {
    /// 从应用数据目录装载账号；目录不存在时以空状态启动。
    pub fn load(app_data_dir: PathBuf) -> Result<Self, AccountError> {
        super::migration::validate_metadata(&app_data_dir)?;
        let store = CredentialStore::new(&app_data_dir);
        let accounts = store.load_accounts();
        store.recover_pending_removals(&accounts);
        let current = store
            .current()
            .filter(|key| accounts.iter().any(|account| &account.account_key == key))
            .or_else(|| accounts.first().map(|account| account.account_key.clone()));
        let status = if current.is_some() {
            AccountStatus::LoggedIn
        } else {
            AccountStatus::LoggedOut
        };

        Ok(Self {
            store,
            mihoyo: MihoyoClient::new()?,
            state: Mutex::new(State {
                status,
                accounts,
                current,
                last_login_failure: None,
                login_platform: None,
            }),
        })
    }
}

impl AccountService for FsAccountService {
    fn authed_get<'a>(
        &'a self,
        account_key: &'a str,
        path: &'a str,
        query: &'a [(String, String)],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, AccountError>> + Send + 'a>> {
        Box::pin(async move {
            let platform = self
                .lock()
                .accounts
                .iter()
                .find(|a| a.account_key == account_key)
                .map(|a| a.platform)
                .ok_or_else(|| AccountError::AccountNotFound(account_key.to_owned()))?;
            // Restrict authenticated requests to validated paths on the fixed API host.
            if !path.starts_with('/')
                || path.starts_with("//")
                || path.contains("..")
                || !path
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/_-".contains(&b))
            {
                return Err(AccountError::InvalidInput);
            }
            let cookies = select_cookies(platform, &self.store.load_credentials(account_key)?);
            let header = crate::mihoyo::cookie_header_for(&cookies, platform.api_host(), path);
            if header.is_empty() {
                return Err(AccountError::NotLoggedIn);
            }
            let bytes = crate::platform_http::HttpClient::new()?
                .get_bytes(
                    &format!("{}{path}", platform.api_origin()),
                    &[
                        ("cookie", &header),
                        ("accept", "application/json, text/plain, */*"),
                        ("origin", platform.page_origin()),
                        ("referer", &format!("{}/", platform.page_origin())),
                    ],
                    query,
                )
                .await?;
            if !self
                .lock()
                .accounts
                .iter()
                .any(|a| a.account_key == account_key)
            {
                return Err(AccountError::AccountNotFound(account_key.to_owned()));
            }
            let envelope: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| AccountError::InvalidResponse)?;
            if envelope.get("retcode").and_then(|v| v.as_i64()) == Some(-100) {
                return Err(AccountError::SessionExpired);
            }
            Ok(bytes)
        })
    }

    fn snapshot(&self) -> AccountSnapshot {
        let state = self.lock();
        AccountSnapshot {
            status: state.status,
            current_account_key: state.current.clone(),
            accounts: state.accounts.clone(),
            last_login_failure: state.last_login_failure,
        }
    }

    fn begin_login(&self, platform: Platform) -> Result<(), AccountError> {
        let mut state = self.lock();
        if state.status.is_logging_in() {
            return Err(AccountError::LoginInProgress);
        }
        state.status = AccountStatus::LoggingIn;
        state.last_login_failure = None;
        state.login_platform = Some(platform);
        Ok(())
    }

    fn submit_login(&self, cookies: Vec<StoredCookie>) -> Result<LoginOutcome, AccountError> {
        let mut state = self.lock();
        if !state.status.is_logging_in() {
            return Err(AccountError::NoLoginInProgress);
        }

        let platform = state
            .login_platform
            .ok_or(AccountError::NoLoginInProgress)?;
        let cookies = select_cookies(platform, &cookies);
        let Some(credentials) = evaluate(platform, &cookies) else {
            return Ok(LoginOutcome::Pending);
        };

        let account_key = platform.account_key(&credentials.account_id)?;
        let now = unix_now();
        let mut accounts = state.accounts.clone();
        match accounts
            .iter_mut()
            .find(|account| account.account_key == account_key)
        {
            Some(account) => {
                if account.platform != platform
                    || account
                        .account_id
                        .as_ref()
                        .is_some_and(|id| id != &credentials.account_id)
                {
                    return Err(AccountError::InvalidResponse);
                }
                account.account_id = Some(credentials.account_id.clone());
                account.mid = credentials.mid.clone().or_else(|| account.mid.clone());
                account.updated_at = now;
            }
            None => accounts.push(Account {
                account_key: account_key.clone(),
                platform,
                account_id: Some(credentials.account_id.clone()),
                mid: credentials.mid.clone(),
                game_roles: Vec::new(),
                avatar_url: None,
                creator_level: None,
                creator_exp: None,
                creator_exp_total: None,
                updated_at: now,
            }),
        }
        self.store.save_credentials(&account_key, &cookies)?;
        let current = Some(account_key.as_str());
        self.store.save_account_state(&accounts, current)?;
        state.accounts = accounts;
        state.current = current.map(str::to_owned);
        state.status = AccountStatus::LoggedIn;
        state.login_platform = None;

        Ok(LoginOutcome::Completed { account_key })
    }

    fn sync_account_info<'a>(
        &'a self,
        account_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Account, AccountError>> + Send + 'a>> {
        Box::pin(async move {
            let previous = self
                .lock()
                .accounts
                .iter()
                .find(|a| a.account_key == account_key)
                .cloned()
                .ok_or_else(|| AccountError::AccountNotFound(account_key.to_owned()))?;
            let platform = previous.platform;
            let cookies = select_cookies(platform, &self.store.load_credentials(account_key)?);
            if cookies.is_empty() {
                return Err(AccountError::NotLoggedIn);
            }

            let mut roles = self.mihoyo.game_roles(platform, &cookies).await?;
            let hours = match self.mihoyo.home(platform, &cookies).await {
                Ok(home) => crate::mihoyo::refresh_hours(&home),
                Err(error) => {
                    warn!(code = error.code(), "数据刷新时间暂未同步");
                    Default::default()
                }
            };
            // Metadata belongs to each explicit UID/region; never switch the website's active role.
            for role in &mut roles {
                let old = previous
                    .game_roles
                    .iter()
                    .find(|old| old.uid == role.uid && old.region == role.region);
                role.creator = old.and_then(|r| r.creator.clone());
                role.data_refresh_hour = hours
                    .get(&role.region)
                    .copied()
                    .or_else(|| old.and_then(|r| r.data_refresh_hour));
                match self
                    .mihoyo
                    .creator_profile(platform, &cookies, &role.uid, &role.region)
                    .await
                {
                    Ok(profile) => role.creator = Some(profile),
                    Err(error) => warn!(code = error.code(), "角色创作者资料暂未同步"),
                }
            }
            let profile = roles.first().and_then(|r| r.creator.clone());

            let mut state = self.lock();
            let mut accounts = state.accounts.clone();
            let account = accounts
                .iter_mut()
                .find(|account| account.account_key == account_key)
                .ok_or_else(|| AccountError::AccountNotFound(account_key.to_owned()))?;
            account.game_roles = roles;
            if let Some(profile) = profile {
                if profile.avatar_url.is_some() {
                    account.avatar_url = profile.avatar_url;
                }
                account.creator_level = profile.level;
                account.creator_exp = profile.exp;
                account.creator_exp_total = profile.exp_total;
            }
            account.updated_at = unix_now();
            let updated = account.clone();
            self.store
                .save_account_state(&accounts, state.current.as_deref())?;
            state.accounts = accounts;
            Ok(updated)
        })
    }

    fn cancel_login(&self, reason: LoginCancelReason) {
        let mut state = self.lock();
        if !state.status.is_logging_in() {
            return;
        }
        state.status = if state.current.is_some() {
            AccountStatus::LoggedIn
        } else {
            AccountStatus::LoggedOut
        };
        state.last_login_failure = Some(reason);
        state.login_platform = None;
    }

    fn remove_account(&self, account_key: &str) -> Result<(), AccountError> {
        let mut state = self.lock();
        let index = state
            .accounts
            .iter()
            .position(|account| account.account_key == account_key)
            .ok_or_else(|| AccountError::AccountNotFound(account_key.to_owned()))?;

        let mut accounts = state.accounts.clone();
        accounts.remove(index);
        let current = if state.current.as_deref() == Some(account_key) {
            accounts.first().map(|account| account.account_key.clone())
        } else {
            state.current.clone()
        };
        self.store
            .remove_account_state(account_key, &accounts, current.as_deref())?;
        state.accounts = accounts;
        state.current = current;
        state.status = if state.current.is_some() {
            AccountStatus::LoggedIn
        } else {
            AccountStatus::LoggedOut
        };
        Ok(())
    }

    fn switch_account(&self, account_key: &str) -> Result<(), AccountError> {
        let mut state = self.lock();
        if !state
            .accounts
            .iter()
            .any(|account| account.account_key == account_key)
        {
            return Err(AccountError::AccountNotFound(account_key.to_owned()));
        }
        self.store
            .save_account_state(&state.accounts, Some(account_key))?;
        state.current = Some(account_key.to_owned());
        state.status = AccountStatus::LoggedIn;
        Ok(())
    }
}

impl FsAccountService {
    /// Cookie presence is only a candidate. Verify it against the selected center before saving.
    pub async fn validate_login(
        &self,
        platform: Platform,
        cookies: &[StoredCookie],
    ) -> Result<(), AccountError> {
        let candidate = evaluate(platform, cookies).ok_or(AccountError::NotLoggedIn)?;
        let home = self
            .mihoyo
            .home(platform, &select_cookies(platform, cookies))
            .await?;
        let account_id = home
            .get("aid")
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| value.as_u64().map(|id| id.to_string()))
            })
            .ok_or(AccountError::InvalidResponse)?;
        if account_id != candidate.account_id {
            return Err(AccountError::InvalidResponse);
        }
        Ok(())
    }

    /// 锁中毒也继续工作：账号状态不可因一次 panic 而整体失效。
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}
