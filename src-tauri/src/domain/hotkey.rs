use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::error::{AppError, ErrorCode};

/// Arguments for `hotkey_update` (`C4.2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyUpdateArgs {
    pub combo: String,
}

/// Parsed and validated global hotkey combination (`D-0004`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHotkey {
    pub normalized: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    pub key: String,
    /// Carbon virtual key code (`kVK_*`).
    pub macos_key_code: u32,
    /// Carbon modifier mask (`cmdKey | shiftKey | optionKey | controlKey`).
    pub macos_modifiers: u32,
}

// Carbon Event Manager modifier masks (from HIToolbox/Events.h).
const CARBON_CMD_KEY: u32 = 1 << 8;
const CARBON_SHIFT_KEY: u32 = 1 << 9;
const CARBON_OPTION_KEY: u32 = 1 << 11;
const CARBON_CONTROL_KEY: u32 = 1 << 12;

/// Parses and validates a hotkey combo string like `"ctrl+alt+space"` or `"ctrl+alt+cmd+space"`.
pub fn parse_hotkey_combo(combo: &str) -> Result<ParsedHotkey, AppError> {
    let trimmed = combo.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            ErrorCode::ValidationError,
            "Hotkey combo must not be empty",
        ));
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut cmd = false;
    let mut primary_key: Option<(&'static str, u32)> = None;

    for raw_part in trimmed.split('+') {
        let token = raw_part.trim().to_ascii_lowercase();
        if token.is_empty() {
            return Err(AppError::with_detail(
                ErrorCode::ValidationError,
                "Hotkey combo contains an empty token",
                combo,
            ));
        }

        match token.as_str() {
            "ctrl" | "control" => {
                if ctrl {
                    return Err(AppError::with_detail(
                        ErrorCode::ValidationError,
                        "Duplicate ctrl modifier in hotkey combo",
                        combo,
                    ));
                }
                ctrl = true;
            }
            "alt" | "opt" | "option" => {
                if alt {
                    return Err(AppError::with_detail(
                        ErrorCode::ValidationError,
                        "Duplicate alt/option modifier in hotkey combo",
                        combo,
                    ));
                }
                alt = true;
            }
            "shift" => {
                if shift {
                    return Err(AppError::with_detail(
                        ErrorCode::ValidationError,
                        "Duplicate shift modifier in hotkey combo",
                        combo,
                    ));
                }
                shift = true;
            }
            "cmd" | "command" | "super" | "meta" => {
                if cmd {
                    return Err(AppError::with_detail(
                        ErrorCode::ValidationError,
                        "Duplicate cmd modifier in hotkey combo",
                        combo,
                    ));
                }
                cmd = true;
            }
            other => {
                if primary_key.is_some() {
                    return Err(AppError::with_detail(
                        ErrorCode::ValidationError,
                        "Hotkey combo must specify exactly one primary key",
                        combo,
                    ));
                }
                primary_key = Some(map_primary_key(other, combo)?);
            }
        }
    }

    if !ctrl && !alt && !shift && !cmd {
        return Err(AppError::with_detail(
            ErrorCode::ValidationError,
            "Hotkey combo must include at least one modifier key",
            combo,
        ));
    }

    let (canonical_key, macos_key_code) = primary_key.ok_or_else(|| {
        AppError::with_detail(
            ErrorCode::ValidationError,
            "Hotkey combo must include a primary key",
            combo,
        )
    })?;

    let mut parts = Vec::with_capacity(5);
    let mut macos_modifiers = 0u32;
    if ctrl {
        parts.push("ctrl");
        macos_modifiers |= CARBON_CONTROL_KEY;
    }
    if alt {
        parts.push("alt");
        macos_modifiers |= CARBON_OPTION_KEY;
    }
    if shift {
        parts.push("shift");
        macos_modifiers |= CARBON_SHIFT_KEY;
    }
    if cmd {
        parts.push("cmd");
        macos_modifiers |= CARBON_CMD_KEY;
    }
    parts.push(canonical_key);

    Ok(ParsedHotkey {
        normalized: parts.join("+"),
        ctrl,
        alt,
        shift,
        cmd,
        key: canonical_key.to_string(),
        macos_key_code,
        macos_modifiers,
    })
}

fn map_primary_key(token: &str, original_combo: &str) -> Result<(&'static str, u32), AppError> {
    let mapped = match token {
        "space" => ("space", 49),
        "return" | "enter" => ("enter", 36),
        "tab" => ("tab", 48),
        "escape" | "esc" => ("esc", 53),
        "a" => ("a", 0),
        "b" => ("b", 11),
        "c" => ("c", 8),
        "d" => ("d", 2),
        "e" => ("e", 14),
        "f" => ("f", 3),
        "g" => ("g", 5),
        "h" => ("h", 4),
        "i" => ("i", 34),
        "j" => ("j", 38),
        "k" => ("k", 40),
        "l" => ("l", 37),
        "m" => ("m", 46),
        "n" => ("n", 45),
        "o" => ("o", 31),
        "p" => ("p", 35),
        "q" => ("q", 12),
        "r" => ("r", 15),
        "s" => ("s", 1),
        "t" => ("t", 17),
        "u" => ("u", 32),
        "v" => ("v", 9),
        "w" => ("w", 13),
        "x" => ("x", 7),
        "y" => ("y", 16),
        "z" => ("z", 6),
        "0" => ("0", 29),
        "1" => ("1", 18),
        "2" => ("2", 19),
        "3" => ("3", 20),
        "4" => ("4", 21),
        "5" => ("5", 23),
        "6" => ("6", 22),
        "7" => ("7", 26),
        "8" => ("8", 28),
        "9" => ("9", 25),
        _ => {
            return Err(AppError::with_detail(
                ErrorCode::ValidationError,
                "Unsupported primary key in hotkey combo",
                original_combo,
            ))
        }
    };
    Ok(mapped)
}
