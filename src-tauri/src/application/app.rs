use crate::domain::app::{ActivationPolicy, AppInfo};

/// Builds the baseline `AppInfo` snapshot for M0-01 bootstrap.
pub fn build_app_info(version: &str, platform: &str) -> AppInfo {
    AppInfo::new(version, platform, None, ActivationPolicy::Accessory)
}
