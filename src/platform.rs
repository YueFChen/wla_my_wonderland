//! Provider configuration belongs to the plugin, independently of the Core transport.
use crate::account::AccountError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    #[default]
    Cn,
    Global,
}

impl Platform {
    pub fn api_origin(self) -> &'static str {
        match self {
            Self::Cn => "https://api-micreator.mihoyo.com",
            Self::Global => "https://sg-api-hoyocreators.hoyoverse.com",
        }
    }

    pub fn api_host(self) -> &'static str {
        match self {
            Self::Cn => "api-micreator.mihoyo.com",
            Self::Global => "sg-api-hoyocreators.hoyoverse.com",
        }
    }

    pub fn page_origin(self) -> &'static str {
        match self {
            Self::Cn => "https://act.mihoyo.com",
            Self::Global => "https://act.hoyoverse.com",
        }
    }

    pub fn game_biz(self) -> &'static str {
        match self {
            Self::Cn => "hk4e_cn",
            Self::Global => "hk4e_global",
        }
    }

    pub fn login_url(self) -> &'static str {
        match self {
            Self::Cn => "https://act.mihoyo.com/miliastra_wonderland/developer?lang=zh-cn",
            Self::Global => "https://act.hoyoverse.com/miliastra_wonderland/developer?lang=zh-cn",
        }
    }

    pub fn browser_profile(self) -> &'static str {
        match self {
            Self::Cn => "mihoyo-login",
            Self::Global => "hoyoverse-login",
        }
    }

    pub fn login_title(self) -> &'static str {
        match self {
            Self::Cn => "登录米哈游通行证 · 国服",
            Self::Global => "登录 HoYoverse 账号 · 国际服",
        }
    }

    pub fn accepts_domain(self, domain: &str) -> bool {
        let domain = domain.trim_start_matches('.').to_ascii_lowercase();
        let roots = match self {
            Self::Cn => ["mihoyo.com", "miyoushe.com"],
            Self::Global => ["hoyoverse.com", "hoyolab.com"],
        };
        roots
            .iter()
            .any(|root| domain == *root || domain.ends_with(&format!(".{root}")))
    }

    /// Keep every former CN key/path; new international identities have their own namespace.
    pub fn account_key(self, account_id: &str) -> Result<String, AccountError> {
        if account_id.is_empty()
            || account_id.len() > 80
            || !account_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(AccountError::InvalidInput);
        }
        Ok(match self {
            Self::Cn => account_id.to_owned(),
            Self::Global => format!("global-{account_id}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_have_distinct_identities_and_cookie_domains() {
        assert_eq!(Platform::Cn.account_key("123").unwrap(), "123");
        assert_eq!(Platform::Global.account_key("123").unwrap(), "global-123");
        assert!(Platform::Global.accepts_domain(".HoYoVerse.com"));
        assert!(Platform::Global.accepts_domain("account.hoyolab.com"));
        assert!(!Platform::Global.accepts_domain("mihoyo.com"));
        assert!(!Platform::Global.accepts_domain("hoyoverse.com.attacker.test"));
        assert!(!Platform::Cn.accepts_domain("hoyoverse.com"));
        assert!(Platform::Global.account_key("../123").is_err());
    }
}
