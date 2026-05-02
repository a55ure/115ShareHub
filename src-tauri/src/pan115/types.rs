use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_flexible_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexI64 {
        Num(i64),
        Str(String),
    }

    match FlexI64::deserialize(deserializer)? {
        FlexI64::Num(n) => Ok(n),
        FlexI64::Str(s) => s.parse::<i64>().map_err(serde::de::Error::custom),
    }
}

fn deserialize_flexible_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexString {
        Str(String),
        Num(i64),
    }

    match FlexString::deserialize(deserializer)? {
        FlexString::Str(s) => Ok(s),
        FlexString::Num(n) => Ok(n.to_string()),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareSnapItem {
    #[serde(rename = "n", default)]
    pub name: String,
    #[serde(rename = "s", deserialize_with = "deserialize_flexible_i64", default)]
    pub size: i64,
    #[serde(rename = "fc", default)]
    pub is_file: i32,
    #[serde(rename = "sha", default)]
    pub sha1: String,
    #[serde(rename = "fid", default)]
    pub file_id: String,
    #[serde(rename = "cid", deserialize_with = "deserialize_flexible_string", default)]
    pub category_id: String,
    #[serde(rename = "pid", default)]
    pub parent_id: String,
    #[serde(rename = "ico", default)]
    pub ico: String,
    #[serde(rename = "t", default)]
    pub update_time: String,
    #[serde(rename = "u", default)]
    pub thumb_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareSnapResponse {
    pub state: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub errno: i64,
    pub data: Option<ShareSnapData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareSnapData {
    #[serde(deserialize_with = "deserialize_flexible_i64", default)]
    pub count: i64,
    #[serde(default)]
    pub list: Vec<ShareSnapItem>,
    #[serde(default)]
    pub userinfo: Option<ShareUserInfo>,
    #[serde(default)]
    pub shareinfo: Option<ShareInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShareUserInfo {
    #[serde(deserialize_with = "deserialize_flexible_string", default)]
    pub user_id: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub face: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShareInfo {
    #[serde(default)]
    pub snap_id: String,
    #[serde(default)]
    pub share_title: String,
    #[serde(default)]
    pub share_state: Option<i64>,
    #[serde(default)]
    pub create_time: Option<i64>,
}
