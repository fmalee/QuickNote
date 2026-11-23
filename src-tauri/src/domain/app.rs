use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::error::{AppError, ErrorCode};

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

/// IPC arguments for `app_open_external_url` (`C2.1`, `C2.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenExternalUrlArgs {
    pub url: String,
}

/// IPC arguments for `app_set_menu_accelerators` (`M2-QA`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetMenuAcceleratorsArgs {
    pub entries: BTreeMap<String, Option<String>>,
}

/// Validates and normalizes an external URL before passing it to the OS opener (`C2.1`).
pub fn validate_external_url(raw: &str) -> Result<String, AppError> {
    if raw.chars().any(|ch| ch.is_control()) {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "External URL must not contain control characters",
        ));
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "External URL must not be empty",
        ));
    }
    if trimmed.starts_with('-') {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "External URL must not start with '-'",
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://")
        && !lower.starts_with("https://")
        && !lower.starts_with("mailto:")
    {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Only http://, https://, and mailto: URLs are allowed",
        ));
    }
    Ok(trimmed.to_string())
}

/// Converts a normalized frontend key combo (e.g. `alt+cmd+c`) into an OS menu accelerator string.
pub fn normalize_combo_to_tauri_accelerator(combo: &str) -> Option<String> {
    let trimmed = combo.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut has_cmd = false;
    let mut primary_key: Option<String> = None;

    for raw_part in trimmed.split('+') {
        let token = raw_part.trim().to_ascii_lowercase();
        if token.is_empty() {
            return None;
        }
        match token.as_str() {
            "ctrl" | "control" => has_ctrl = true,
            "alt" | "option" | "opt" => has_alt = true,
            "shift" => has_shift = true,
            "cmd" | "command" | "meta" | "mod" | "cmdorctrl" | "super" => has_cmd = true,
            _ => {
                if primary_key.is_some() {
                    return None;
                }
                let mapped = map_primary_key_to_accelerator(&token)?;
                primary_key = Some(mapped);
            }
        }
    }

    let key = primary_key?;
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    if has_ctrl {
        parts.push("Ctrl");
    }
    if has_alt {
        parts.push("Alt");
    }
    if has_shift {
        parts.push("Shift");
    }
    if has_cmd {
        parts.push("CmdOrCtrl");
    }
    parts.push(&key);
    Some(parts.join("+"))
}

fn map_primary_key_to_accelerator(token: &str) -> Option<String> {
    if token.len() == 1 {
        let ch = token.chars().next()?;
        if ch.is_ascii_alphabetic() {
            return Some(ch.to_ascii_uppercase().to_string());
        }
        if ch.is_ascii_digit() {
            return Some(ch.to_string());
        }
        if matches!(
            ch,
            ',' | '.' | '/' | '\\' | '[' | ']' | ';' | '\'' | '`' | '-' | '='
        ) {
            return Some(ch.to_string());
        }
        return None;
    }

    match token {
        "space" | "spacebar" => Some("Space".to_string()),
        "enter" | "return" => Some("Enter".to_string()),
        "escape" | "esc" => Some("Escape".to_string()),
        "tab" => Some("Tab".to_string()),
        "backspace" => Some("Backspace".to_string()),
        "delete" | "del" => Some("Delete".to_string()),
        "arrowup" | "up" => Some("Up".to_string()),
        "arrowdown" | "down" => Some("Down".to_string()),
        "arrowleft" | "left" => Some("Left".to_string()),
        "arrowright" | "right" => Some("Right".to_string()),
        "comma" => Some(",".to_string()),
        "period" => Some(".".to_string()),
        _ => {
            if let Some(rest) = token.strip_prefix('f') {
                if let Ok(num) = rest.parse::<u8>() {
                    if (1..=24).contains(&num) {
                        return Some(format!("F{num}"));
                    }
                }
            }
            None
        }
    }
}
