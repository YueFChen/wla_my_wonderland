//! Select one provider's cookies and derive its official account identity.

use crate::Platform;
use crate::account::StoredCookie;

/// 凭据齐全后提取出的账号标识。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    /// 米哈游通行证账号 ID。
    pub account_id: String,
    /// 米游社 mid；仅 V2 cookie 提供。
    pub mid: Option<String>,
}

pub fn select_cookies(platform: Platform, cookies: &[StoredCookie]) -> Vec<StoredCookie> {
    cookies
        .iter()
        .filter(|cookie| platform.accepts_domain(&cookie.domain))
        .cloned()
        .collect()
}

/// 令牌键：`cookie_token` / `ltoken` / `stoken` 及其 V2 变体。
fn is_token(name: &str) -> bool {
    matches!(
        name,
        "cookie_token" | "cookie_token_v2" | "ltoken" | "ltoken_v2" | "stoken" | "stoken_v2"
    )
}

/// 凭据是否齐全；齐全时返回账号标识。
///
/// Follow the official SDK's ID priority; the center subsequently verifies the candidate.
pub fn evaluate(platform: Platform, cookies: &[StoredCookie]) -> Option<Credentials> {
    let cookies = select_cookies(platform, cookies);
    let account_id = ["account_id_v2", "account_id", "ltuid_v2", "ltuid"]
        .iter()
        .find_map(|name| {
            cookies
                .iter()
                .find(|cookie| cookie.name == *name && !cookie.value.is_empty())
        })?
        .value
        .clone();
    if platform.account_key(&account_id).is_err()
        || !cookies
            .iter()
            .any(|cookie| is_token(&cookie.name) && !cookie.value.is_empty())
    {
        return None;
    }

    let mid = cookies
        .iter()
        .find(|cookie| matches!(cookie.name.as_str(), "account_mid_v2" | "ltmid_v2" | "mid"))
        .map(|cookie| cookie.value.clone())
        .filter(|value| !value.is_empty());

    Some(Credentials { account_id, mid })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cookie(name: &str, value: &str, domain: &str) -> StoredCookie {
        StoredCookie {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
            path: "/".into(),
        }
    }

    #[test]
    fn mixed_domains_never_mix_provider_identities_or_empty_tokens() {
        let cookies = vec![
            cookie("ltuid", "123", ".mihoyo.com"),
            cookie("ltoken", "cn-token", ".mihoyo.com"),
            cookie("ltuid_v2", "456", "hoyoverse.com"),
            cookie("ltoken_v2", "global-token", ".hoyoverse.com"),
        ];
        assert_eq!(evaluate(Platform::Cn, &cookies).unwrap().account_id, "123");
        assert_eq!(
            evaluate(Platform::Global, &cookies).unwrap().account_id,
            "456"
        );
        assert!(
            evaluate(
                Platform::Global,
                &[
                    cookie("account_id", "456", ".hoyoverse.com"),
                    cookie("ltoken_v2", "", ".hoyoverse.com")
                ]
            )
            .is_none()
        );
        assert!(evaluate(Platform::Cn, &cookies[2..]).is_none());
    }

    #[test]
    fn official_account_marker_precedes_legacy_ids_and_lookalike_names_are_ignored() {
        let cookies = vec![
            cookie("ltuid", "123", ".hoyoverse.com"),
            cookie("account_id_v2", "456", ".hoyoverse.com"),
            cookie("ltoken_v2", "token", ".hoyoverse.com"),
        ];
        assert_eq!(
            evaluate(Platform::Global, &cookies).unwrap().account_id,
            "456"
        );
        assert!(
            evaluate(
                Platform::Global,
                &[
                    cookie("account_id_fake", "456", ".hoyoverse.com"),
                    cookie("ltoken_fake", "token", ".hoyoverse.com"),
                ]
            )
            .is_none()
        );
    }
}
