use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GameEntry {
    pub appid: String,
    pub name: Option<String>,
    pub path: String,
    pub build_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ItemStatus {
    Unknown,
    UpToDate,
    OutOfDate,
}

#[derive(Debug, Clone)]
pub struct WorkshopItem {
    pub item_id: String,
    pub name: Option<String>,
    pub path: PathBuf,

    pub local_timestamp: Option<u64>,
    pub remote_timestamp: Option<u64>,

    pub disk_size: u64,

    pub status: ItemStatus,
    pub incompatible: bool,
    pub supported_versions: Vec<String>,
    pub preview_url: Option<String>,

    pub selected: bool,
}
