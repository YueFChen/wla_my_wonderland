//! 平台接口客户端：统一的 cookie 组装与调用入口。

use crate::Platform;
use crate::account::{AccountError, CreatorProfile, StoredCookie};
use crate::platform_http::HttpClient;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};

use super::creator::{BasicInfo, Envelope, LevelDetail, non_zero};
use super::endpoint::{
    API_TAKUMI, GAME_BIZ_HK4E_CN, PATH_CREATOR_BASIC_INFO, PATH_CREATOR_HOME,
    PATH_CREATOR_LEVEL_DETAIL, PATH_CREATOR_ROLES, PATH_GAME_ROLES,
};
use super::role::{CreatorRoleList, GameRole, RoleListResponse};

/// 米哈游平台接口客户端。
///
/// 只负责"怎么请求"，不持有凭据：调用方每次传入 cookie 快照。
pub struct MihoyoClient {
    http: HttpClient,
}

impl MihoyoClient {
    pub fn new() -> Result<Self, AccountError> {
        Ok(Self {
            http: HttpClient::new()?,
        })
    }

    /// 拉取该账号绑定的原神角色。
    pub async fn game_roles(
        &self,
        platform: Platform,
        cookies: &[StoredCookie],
    ) -> Result<Vec<GameRole>, AccountError> {
        if cookies.is_empty() {
            return Err(AccountError::NotLoggedIn);
        }

        if platform == Platform::Global {
            let response: CreatorRoleList = self
                .creator_get(platform, cookies, PATH_CREATOR_ROLES, "", "", &[])
                .await?;
            let roles = response
                .list
                .into_iter()
                .map(GameRole::try_from)
                .collect::<Result<Vec<_>, _>>()?;
            let mut seen = BTreeSet::new();
            if roles
                .iter()
                .any(|role| !seen.insert((&role.uid, &role.region)))
            {
                return Err(AccountError::InvalidResponse);
            }
            return Ok(roles);
        }

        let cookie = cookie_header_for(cookies, "api-takumi.mihoyo.com", PATH_GAME_ROLES);
        let url = format!("{API_TAKUMI}{PATH_GAME_ROLES}?game_biz={GAME_BIZ_HK4E_CN}");

        let response: RoleListResponse = self
            .http
            .get_json(
                &url,
                &[("cookie", cookie.as_str()), ("accept", ACCEPT_JSON)],
            )
            .await?;

        if response.retcode != 0 {
            return Err(response_error(response.retcode, response.message));
        }

        Ok(response
            .data
            .ok_or(AccountError::InvalidResponse)?
            .list
            .into_iter()
            .filter(|item| item.game_biz == GAME_BIZ_HK4E_CN)
            .map(GameRole::from)
            .collect())
    }

    /// 拉取创作者中心资料：游戏内昵称 / 头像与奇匠等级。
    ///
    /// 头像只能从这里拿到——原神战绩接口的 `AvatarUrl` 恒空，登录页图标又依赖 DOM 渲染时机，
    /// 因此账号页的头像以本接口为准。两次 GET 都是只读，失败由调用方决定是否降级。
    pub async fn creator_profile(
        &self,
        platform: Platform,
        cookies: &[StoredCookie],
        uid: &str,
        region: &str,
    ) -> Result<CreatorProfile, AccountError> {
        if cookies.is_empty() {
            return Err(AccountError::NotLoggedIn);
        }

        let basic: BasicInfo = self
            .creator_get(
                platform,
                cookies,
                PATH_CREATOR_BASIC_INFO,
                uid,
                region,
                &[("need_ra_game_info", "true")],
            )
            .await?;
        let level: LevelDetail = self
            .creator_get(
                platform,
                cookies,
                PATH_CREATOR_LEVEL_DETAIL,
                uid,
                region,
                &[],
            )
            .await?;

        let game = basic.ra_game_info.unwrap_or_default();
        let avatar_url = (!game.avatar_url.is_empty()).then_some(game.avatar_url);
        Ok(CreatorProfile {
            avatar_url,
            level: non_zero(level.cur_level).or_else(|| non_zero(basic.kolugc_level)),
            exp: Some(level.cur_exp),
            exp_total: Some(level.total_exp),
        })
    }

    pub async fn home(
        &self,
        platform: Platform,
        cookies: &[StoredCookie],
    ) -> Result<serde_json::Value, AccountError> {
        self.creator_get(platform, cookies, PATH_CREATOR_HOME, "", "", &[])
            .await
    }

    /// Both centers use the same business paths. Only the provider configuration differs.
    async fn creator_get<T: DeserializeOwned>(
        &self,
        platform: Platform,
        cookies: &[StoredCookie],
        path: &str,
        uid: &str,
        region: &str,
        extra: &[(&str, &str)],
    ) -> Result<T, AccountError> {
        let cookie = cookie_header_for(cookies, platform.api_host(), path);
        if cookie.is_empty() {
            return Err(AccountError::NotLoggedIn);
        }

        let url = format!("{}{path}", platform.api_origin());
        let mut query = vec![
            ("lang".into(), "zh-cn".into()),
            ("game_biz".into(), platform.game_biz().into()),
        ];
        if !uid.is_empty() {
            query.push(("uid".into(), uid.to_owned()));
        }
        if !region.is_empty() {
            query.push(("region".into(), region.to_owned()));
        }
        for (name, value) in extra {
            query.push(((*name).to_owned(), (*value).to_owned()));
        }

        let bytes = self
            .http
            .get_bytes(
                &url,
                &[
                    ("cookie", cookie.as_str()),
                    ("accept", ACCEPT_JSON),
                    ("origin", platform.page_origin()),
                    ("referer", &format!("{}/", platform.page_origin())),
                ],
                &query,
            )
            .await?;
        let response: Envelope<T> =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::InvalidResponse)?;

        if response.retcode != 0 {
            return Err(response_error(response.retcode, response.message));
        }
        response.data.ok_or(AccountError::InvalidResponse)
    }
}

fn response_error(code: i32, message: Option<String>) -> AccountError {
    if code == -100 {
        AccountError::SessionExpired
    } else {
        AccountError::Business {
            code: i64::from(code),
            message: message.unwrap_or_default(),
        }
    }
}

pub(crate) fn refresh_hours(home: &serde_json::Value) -> BTreeMap<String, u8> {
    home.get("data_center_refresh_hours")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(region, value)| {
            let hour = value
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .or_else(|| value.as_str()?.trim().split(':').next()?.parse().ok())?;
            (hour < 24).then(|| (region.clone(), hour))
        })
        .collect()
}

/// 米哈游接口普遍接受的 `Accept`。
pub(crate) const ACCEPT_JSON: &str = "application/json, text/plain, */*";

/// 把 cookie 快照拼成 `Cookie` 请求头。
///
/// 凭据只在这一处落成字符串，不向上层或日志暴露。
pub fn cookie_header_for(cookies: &[StoredCookie], host: &str, path: &str) -> String {
    let mut selected: Vec<_> = cookies
        .iter()
        .filter(|cookie| {
            let domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
            let domain_match = host.eq_ignore_ascii_case(&domain)
                || ((cookie.domain.starts_with('.')
                    || domain == "mihoyo.com"
                    || domain == "miyoushe.com"
                    || domain == "hoyoverse.com"
                    || domain == "hoyolab.com")
                    && host.ends_with(&format!(".{domain}")));
            let path_match = path == cookie.path
                || (path.starts_with(&cookie.path)
                    && (cookie.path.ends_with('/')
                        || path.as_bytes().get(cookie.path.len()) == Some(&b'/')));
            domain_match && path_match
        })
        .collect();
    selected.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
    selected
        .into_iter()
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_configuration_preserves_per_region_hours_and_rejects_invalid_values() {
        let home = serde_json::json!({"data_center_refresh_hours":{"os_asia":"6:00","os_usa":18,"os_euro":"12","os_cht":0,"bad":25,"missing":null}});
        let result = refresh_hours(&home);
        assert_eq!(result.len(), 4);
        assert_eq!(result["os_asia"], 6);
        assert_eq!(result["os_cht"], 0);
        assert!(refresh_hours(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn normalized_global_parent_cookies_match_only_the_provider_host() {
        let cookie = StoredCookie {
            name: "ltoken_v2".into(),
            value: "synthetic".into(),
            domain: "HoYoVerse.com".into(),
            path: "/".into(),
        };
        assert_eq!(
            cookie_header_for(
                std::slice::from_ref(&cookie),
                Platform::Global.api_host(),
                "/kolugc_hch"
            ),
            "ltoken_v2=synthetic"
        );
        assert!(cookie_header_for(&[cookie], Platform::Cn.api_host(), "/kolugc_hch").is_empty());
    }
    #[test]
    fn webview_normalized_parent_domain_and_path_boundaries() {
        let cookie = |name: &str, domain: &str, path: &str| StoredCookie {
            name: name.into(),
            value: "test-only".into(),
            domain: domain.into(),
            path: path.into(),
        };
        let cookies = vec![
            cookie("token", "mihoyo.com", "/"),
            cookie("other", "passport-api.mihoyo.com", "/"),
            cookie("wrong", "mihoyo.com", "/data"),
            cookie("specific", ".mihoyo.com", "/kolugc_hch"),
        ];
        assert_eq!(
            cookie_header_for(
                &cookies,
                "api-micreator.mihoyo.com",
                "/kolugc_hch/common/v1/data/get_user_info"
            ),
            "specific=test-only; token=test-only"
        );
        assert!(cookie_header_for(&cookies, "mihoyo.com.attacker.test", "/").is_empty());
        assert!(
            !cookie_header_for(&cookies, "api-micreator.mihoyo.com", "/database").contains("wrong")
        );
    }
}
