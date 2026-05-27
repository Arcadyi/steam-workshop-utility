use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "SWUC_";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CollectionCode {
    pub appid: String,
    pub name: String,
    pub items: Vec<String>, // workshop item IDs
}

impl CollectionCode {
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        format!("{}{}", PREFIX, URL_SAFE_NO_PAD.encode(json.as_bytes()))
    }

    pub fn decode(code: &str) -> Option<Self> {
        let stripped = code.trim().strip_prefix(PREFIX)?;
        let bytes = URL_SAFE_NO_PAD.decode(stripped).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
}