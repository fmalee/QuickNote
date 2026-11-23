use serde::{Deserialize, Serialize};
use specta::Type;

/// macOS application activation policy (`D-0003`, `C3.4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ActivationPolicy {
    Accessory,
    Regular,
}

/// Basic runtime metadata exposed via `app_get_info` (`C3.4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub library_root: Option<String>,
    pub activation_policy: ActivationPolicy,
}

impl AppInfo {
    /// Constructs a new `AppInfo` snapshot.
    pub fn new(
        version: impl Into<String>,
        platform: impl Into<String>,
        library_root: Option<String>,
        activation_policy: ActivationPolicy,
    ) -> Self {
        Self {
            version: version.into(),
            platform: platform.into(),
            library_root,
            activation_policy,
        }
    }
}
