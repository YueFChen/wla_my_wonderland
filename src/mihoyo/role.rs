//! 绑定游戏角色的响应模型。
//!
//! 外部接口字段会随版本增删，可选字段一律给默认值，避免解析直接失败。

pub use crate::account::GameRole;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RoleListResponse {
    pub(crate) retcode: i32,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) data: Option<RoleListData>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RoleListData {
    #[serde(default)]
    pub(crate) list: Vec<RoleListItem>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RoleListItem {
    #[serde(default)]
    pub(crate) game_biz: String,
    #[serde(default)]
    game_uid: String,
    #[serde(default)]
    region: String,
    #[serde(default)]
    region_name: String,
    #[serde(default)]
    nickname: String,
    #[serde(default)]
    level: u32,
}

impl From<RoleListItem> for GameRole {
    fn from(item: RoleListItem) -> Self {
        Self {
            uid: item.game_uid,
            region: item.region,
            region_name: item.region_name,
            nickname: item.nickname,
            level: item.level,
            creator: None,
            data_refresh_hour: None,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreatorRoleList {
    pub list: Vec<CreatorRole>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreatorRole {
    #[serde(alias = "game_uid")]
    pub uid: String,
    pub region: String,
    #[serde(default)]
    pub region_name: String,
    #[serde(default, alias = "nickname")]
    pub role_nickname: String,
    #[serde(default, alias = "level")]
    pub role_level: u32,
}

impl TryFrom<CreatorRole> for GameRole {
    type Error = crate::account::AccountError;
    fn try_from(role: CreatorRole) -> Result<Self, Self::Error> {
        if role.uid.is_empty() || role.region.is_empty() {
            return Err(Self::Error::InvalidResponse);
        }
        let region_name = if role.region_name.is_empty() {
            match role.region.as_str() {
                "os_usa" => "美服",
                "os_euro" => "欧服",
                "os_asia" => "亚服",
                "os_cht" => "台港澳服",
                _ => &role.region,
            }
            .to_owned()
        } else {
            role.region_name
        };
        Ok(Self {
            uid: role.uid,
            region: role.region,
            region_name,
            nickname: role.role_nickname,
            level: role.role_level,
            creator: None,
            data_refresh_hour: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creator_center_role_fields_and_all_global_regions_are_supported() {
        let data: CreatorRoleList = serde_json::from_value(serde_json::json!({"list": [
            {"uid":"1800000001","region":"os_asia","role_nickname":"Asia role","role_level":5},
            {"uid":"600000001","region":"os_usa","role_nickname":"US role","role_level":30},
            {"uid":"700000001","region":"os_euro","role_nickname":"EU role","role_level":40},
            {"uid":"900000001","region":"os_cht","role_nickname":"TW role","role_level":50}
        ]}))
        .unwrap();
        let roles = data
            .list
            .into_iter()
            .map(GameRole::try_from)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(roles.len(), 4);
        assert_eq!(roles[0].nickname, "Asia role");
        assert_eq!(roles[0].region_name, "亚服");
        assert_eq!(roles[3].level, 50);
        assert!(serde_json::from_value::<CreatorRoleList>(serde_json::json!({})).is_err());
    }
}
