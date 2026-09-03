use serde::{Deserialize, Serialize};

/// Response from dccon package_detail API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DcconApiResponse {
    pub info: DcconInfo,
    pub detail: Vec<DcconImage>,
}

/// Information about a dccon package
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DcconInfo {
    pub title: String,
    pub package_idx: String,
}

/// Individual dccon image information
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DcconImage {
    pub path: String,
    pub ext: String,
}

/// Simplified dccon info for frontend
#[derive(Debug, Serialize, Clone)]
pub struct DcconPackageInfo {
    pub title: String,
    pub package_idx: String,
    pub image_count: usize,
    pub images: Vec<DcconImage>,
}
