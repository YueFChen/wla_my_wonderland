use serde::de::DeserializeOwned;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};
use wonderland_plugin_sdk::PluginFailure;
pub mod account;
mod account_error;
mod mihoyo;
pub mod platform;
pub mod platform_http;
pub use account::{
    Account, AccountSnapshot, AccountStatus, CreatorProfile, GameRole, LoginCancelReason,
};
pub use platform::Platform;
pub mod history;
pub mod model;
pub use history::HistoryImportReport;
pub use model::*;

/// Plugin-owned account access for collection.
pub trait AccountProvider: Send + Sync + 'static {
    fn authed_get<'a>(
        &'a self,
        account_key: &'a str,
        path: &'a str,
        query: &'a [(String, String)],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, PluginFailure>> + Send + 'a>>;

    fn snapshot(&self) -> AccountSnapshot;
}

pub struct MyWonderland {
    root: PathBuf,
    account: Arc<dyn AccountProvider>,
    // 单插件串行采集，避免同一序列并发写入；不阻塞账号切换或读取。
    collecting: Mutex<()>,
}
impl MyWonderland {
    /// Construct the business service with its plugin-owned data directory and plugin-owned account provider.
    pub fn new(root: PathBuf, account: Arc<dyn AccountProvider>) -> Result<Self, PluginFailure> {
        fs::create_dir_all(&root).map_err(storage)?;
        Ok(Self {
            root,
            account,
            collecting: Mutex::new(()),
        })
    }
}
/// 底层 IO 原因带进 `details`，否则只剩一句"读写失败"，排查时无从下手。
fn storage(error: impl std::fmt::Display) -> PluginFailure {
    warn!(reason = %error, "本地序列读写失败");
    PluginFailure::Archive(error.to_string())
}
fn safe(value: &str) -> bool {
    !value.is_empty()
        && value.len() < 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn scope_valid(scope: &Scope) -> Result<(), PluginFailure> {
    if [&scope.account_key, &scope.uid, &scope.region]
        .iter()
        .all(|s| safe(s))
    {
        Ok(())
    } else {
        Err(PluginFailure::InvalidInput)
    }
}
impl MyWonderland {
    fn series_path(&self, scope: &Scope) -> Result<PathBuf, PluginFailure> {
        scope_valid(scope)?;
        Ok(self
            .root
            .join(&scope.account_key)
            .join(&scope.region)
            .join(format!("{}.json", scope.uid)))
    }
    fn load(&self, scope: &Scope) -> Result<Option<Series>, PluginFailure> {
        let path = self.series_path(scope)?;
        if !path.exists() {
            return Ok(None);
        }
        // 下面两种失败都出在**本地文件**上，不能借用 InvalidResponse——
        // 那条错误的文案说的是"官方接口数据结构已变化"，会把用户指向错误的方向。
        let series: Series =
            serde_json::from_slice(&fs::read(&path).map_err(storage)?).map_err(|_| {
                warn!("本地序列文件无法解析");
                PluginFailure::LocalData("本地序列文件无法解析，请重新采集".into())
            })?;
        if series.schema_version != SERIES_VERSION || series.scope != *scope {
            warn!("本地序列版本或账号范围不匹配");
            return Err(PluginFailure::LocalData(
                "本地序列版本或账号范围不匹配，请重新采集".into(),
            ));
        }
        Ok(Some(series))
    }
    fn save(&self, series: &Series) -> Result<(), PluginFailure> {
        let path = self.series_path(&series.scope)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(storage)?;
        }
        let bytes = serde_json::to_vec(series).map_err(storage)?;
        // 半途失败不破坏上一份序列：先写临时文件再原子替换。
        let temp = path.with_extension("tmp");
        fs::write(&temp, bytes).map_err(storage)?;
        fs::rename(temp, path).map_err(storage)
    }
    /// 读取本地长期序列（不触发采集）。
    pub fn series(&self, scope: &Scope) -> Result<Option<Series>, PluginFailure> {
        self.load(scope)
    }
    fn validate_role(&self, scope: &Scope) -> Result<(), PluginFailure> {
        scope_valid(scope)?;
        let snapshot = self.account.snapshot();
        let account = snapshot
            .accounts
            .iter()
            .find(|a| a.account_key == scope.account_key)
            .ok_or_else(|| PluginFailure::AccountNotFound(scope.account_key.clone()))?;
        if account
            .game_roles
            .iter()
            .any(|r| r.uid == scope.uid && r.region == scope.region)
        {
            Ok(())
        } else {
            Err(PluginFailure::InvalidInput)
        }
    }
    async fn fetch<T: DeserializeOwned>(
        &self,
        scope: &Scope,
        endpoint: &str,
        extra: Vec<(String, String)>,
    ) -> Result<T, PluginFailure> {
        self.validate_role(scope)?;
        let platform = self
            .account
            .snapshot()
            .accounts
            .into_iter()
            .find(|account| account.account_key == scope.account_key)
            .ok_or_else(|| PluginFailure::AccountNotFound(scope.account_key.clone()))?
            .platform;
        let mut query = vec![
            ("lang".into(), "zh-cn".into()),
            ("game_biz".into(), platform.game_biz().into()),
            ("uid".into(), scope.uid.clone()),
            ("region".into(), scope.region.clone()),
        ];
        query.extend(extra);
        let path = format!("/kolugc_hch/common/v1/data/{endpoint}");
        let bytes = self
            .account
            .authed_get(&scope.account_key, &path, &query)
            .await?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| PluginFailure::InvalidResponse)?;
        decode(value)
    }
    /// 采集一次并并入本地序列；返回合并后的序列。
    ///
    /// 官方只给滚动窗口，所以这里不做"快照归档"，而是把点位按日期并进已有序列：
    /// 相邻两次采集的重叠部分不会重复存储。
    #[instrument(
        skip(self, scope),
        fields(plugin = "wla_my_wonderland", uid = %scope.uid, region = %scope.region)
    )]
    pub async fn collect(&self, scope: Scope) -> Result<Series, PluginFailure> {
        let _guard = self
            .collecting
            .try_lock()
            .map_err(|_| PluginFailure::Busy)?;
        self.validate_role(&scope)?;
        info!("开始采集奇域数据");
        let overview = self
            .fetch::<Overview>(&scope, "get_user_info", vec![])
            .await?;
        let mut stages = Vec::new();
        let mut seen = BTreeSet::new();
        let mut expected = None;
        for page in 1..=1000 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let result = self
                .fetch::<StagePage>(
                    &scope,
                    "get_stage_list",
                    vec![
                        ("page_index".into(), page.to_string()),
                        ("page_size".into(), "8".into()),
                        ("stage_name".into(), "".into()),
                        (
                            "sort_type".into(),
                            "STAGE_LIST_SORT_BY_POST_TIME_DESC".into(),
                        ),
                    ],
                )
                .await?;
            let complete = append_page(&mut stages, &mut seen, &mut expected, result, &scope)?;
            debug!(page, total = ?expected, "抓取一页作品列表");
            if complete {
                break;
            }
            if page == 1000 {
                return Err(PluginFailure::InvalidResponse);
            }
        }
        let mut details = BTreeMap::new();
        let mut detail_failures = 0;
        for (index, stage) in stages.iter().enumerate() {
            // 官网删除的作品不再提供详情；列表记录仍并入序列，历史点位照旧保留。
            if stage.base_info.game_list_status == "GAME_LIST_STATUS_DELETED" {
                continue;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let id = &stage.base_info.stage_id;
            debug!(stage_id = %id, "抓取作品详情");
            let detail = match self
                .fetch::<Detail>(
                    &scope,
                    "get_stage_detail",
                    vec![("stage_id".into(), id.clone())],
                )
                .await
            {
                Ok(detail) => detail,
                Err(error) => {
                    detail_failures += 1;
                    warn!(stage_id = %id, error = %error, "作品详情获取失败，继续保存其他采集结果");
                    // A transient timeout or stage-specific business error can be
                    // isolated. Shared auth, transport, or response failures apply
                    // to the remaining requests too, so stop instead of repeating them.
                    if matches!(
                        &error,
                        PluginFailure::SessionExpired
                            | PluginFailure::Connection
                            | PluginFailure::Http(_)
                            | PluginFailure::InvalidResponse
                            | PluginFailure::InvalidInput
                            | PluginFailure::NotLoggedIn
                            | PluginFailure::AccountNotFound(_)
                            | PluginFailure::Transport(_)
                            | PluginFailure::Other(_)
                    ) {
                        detail_failures += stages[index + 1..]
                            .iter()
                            .filter(|remaining| {
                                remaining.base_info.game_list_status != "GAME_LIST_STATUS_DELETED"
                            })
                            .count();
                        break;
                    }
                    continue;
                }
            };
            if detail.stage_info.stage_id != *id
                || detail.stage_info.uid != scope.uid
                || detail.stage_info.region != scope.region
            {
                detail_failures += 1;
                warn!(stage_id = %id, "作品详情身份与请求不匹配，跳过该详情");
                continue;
            }
            details.insert(id.clone(), detail);
        }
        // 遗忘账号时，已在途请求不能落新数据。普通切换不影响显式账号的采集。
        self.validate_role(&scope)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(storage)?;
        let mut series = self
            .load(&scope)?
            .unwrap_or_else(|| Series::empty(scope.clone()));
        series.merge(now.as_secs(), overview, stages, details);
        self.save(&series)?;
        info!(
            stages = series.stages.len(),
            detail_failures, "采集结果已保存"
        );
        if detail_failures > 0 {
            return Err(PluginFailure::Other(format!(
                "{detail_failures} 项作品详情暂未获取成功，其余数据已保存；可点击“重试”再次采集。"
            )));
        }
        Ok(series)
    }
}
pub fn decode<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, PluginFailure> {
    let code = value
        .get("retcode")
        .and_then(|v| v.as_i64())
        .ok_or(PluginFailure::InvalidResponse)?;
    if code == -100 {
        return Err(PluginFailure::SessionExpired);
    }
    if code != 0 {
        // 官方文案比裸错误码有用，能取到就带进错误。
        let message = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
        return Err(PluginFailure::Business { code, message });
    }
    let data = value
        .get("data")
        .filter(|v| !v.is_null())
        .ok_or(PluginFailure::InvalidResponse)?;
    serde_json::from_value(data.clone()).map_err(|_| PluginFailure::InvalidResponse)
}
fn append_page(
    stages: &mut Vec<Stage>,
    seen: &mut BTreeSet<String>,
    expected: &mut Option<u32>,
    page: StagePage,
    scope: &Scope,
) -> Result<bool, PluginFailure> {
    if expected.is_some_and(|n| n != page.total) || page.stage_list.len() > 8 {
        return Err(PluginFailure::InvalidResponse);
    }
    *expected = Some(page.total);
    if page.stage_list.is_empty() && stages.len() != page.total as usize {
        return Err(PluginFailure::InvalidResponse);
    }
    for stage in page.stage_list {
        if stage.base_info.uid != scope.uid
            || stage.base_info.region != scope.region
            || !seen.insert(stage.base_info.stage_id.clone())
        {
            return Err(PluginFailure::InvalidResponse);
        }
        stages.push(stage);
    }
    if stages.len() > page.total as usize {
        return Err(PluginFailure::InvalidResponse);
    }
    Ok(stages.len() == page.total as usize)
}
#[cfg(test)]
mod tests;
