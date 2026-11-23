use quicknote_lib::application::app::build_app_info;
use quicknote_lib::bindings::{render_ts_bindings, GENERATED_HEADER};
use quicknote_lib::domain::app::ActivationPolicy;
use quicknote_lib::domain::error::{AppError, ErrorCode};

#[test]
fn error_code_serializes_all_variants_in_camel_case() {
    let cases = [
        (ErrorCode::ValidationError, "validationError"),
        (ErrorCode::LibraryNotConfigured, "libraryNotConfigured"),
        (ErrorCode::NotFound, "notFound"),
        (ErrorCode::PermissionDenied, "permissionDenied"),
        (ErrorCode::Conflict, "conflict"),
        (ErrorCode::UndoUnavailable, "undoUnavailable"),
        (ErrorCode::InvalidFrontmatter, "invalidFrontmatter"),
        (ErrorCode::SettingsParseError, "settingsParseError"),
        (ErrorCode::ExternalAppMissing, "externalAppMissing"),
        (ErrorCode::WorkspaceDirMissing, "workspaceDirMissing"),
        (ErrorCode::TerminalLimitReached, "terminalLimitReached"),
        (ErrorCode::TerminalSpawnFailed, "terminalSpawnFailed"),
        (ErrorCode::InternalError, "internalError"),
    ];

    for (code, expected) in cases {
        let serialized = serde_json::to_value(code).expect("ErrorCode must serialize");
        assert_eq!(serialized, serde_json::Value::String(expected.to_string()));
        let roundtrip: ErrorCode =
            serde_json::from_value(serialized).expect("ErrorCode must deserialize");
        assert_eq!(roundtrip, code);
    }
}

#[test]
fn app_error_roundtrips_with_optional_detail() {
    let err = AppError::with_detail(ErrorCode::Conflict, "body hash mismatch", "expected=abc");
    let json = serde_json::to_value(&err).expect("AppError must serialize");
    assert_eq!(json["code"], "conflict");
    assert_eq!(json["message"], "body hash mismatch");
    assert_eq!(json["detail"], "expected=abc");

    let decoded: AppError = serde_json::from_value(json).expect("AppError must deserialize");
    assert_eq!(decoded, err);

    let simple = AppError::new(ErrorCode::NotFound, "missing file");
    let simple_json = serde_json::to_value(&simple).expect("AppError must serialize");
    assert_eq!(simple_json["detail"], serde_json::Value::Null);
}

#[test]
fn app_get_info_returns_baseline_domain_shape() {
    let built = build_app_info("0.1.0", "macos");
    assert_eq!(built.version, "0.1.0");
    assert_eq!(built.platform, "macos");
    assert_eq!(built.library_root, None);
    assert_eq!(built.activation_policy, ActivationPolicy::Accessory);

    let json = serde_json::to_value(&built).expect("AppInfo must serialize");
    assert_eq!(json["libraryRoot"], serde_json::Value::Null);
    assert_eq!(json["activationPolicy"], "accessory");
}

#[test]
fn specta_bindings_include_required_header_and_types() {
    let first = render_ts_bindings().expect("specta export must succeed");
    let second = render_ts_bindings().expect("specta export must be deterministic");
    assert_eq!(first, second);

    let first_line = first
        .lines()
        .next()
        .expect("rendered output must have lines");
    assert_eq!(first_line, GENERATED_HEADER);

    for needle in [
        "export type ActivationPolicy =",
        "export type AppInfo =",
        "export type AppError =",
        "export type ErrorCode =",
        "libraryRoot",
        "activationPolicy",
        "\"validationError\"",
        "\"libraryNotConfigured\"",
        "\"notFound\"",
        "\"permissionDenied\"",
        "\"conflict\"",
        "\"undoUnavailable\"",
        "\"invalidFrontmatter\"",
        "\"settingsParseError\"",
        "\"externalAppMissing\"",
        "\"workspaceDirMissing\"",
        "\"terminalLimitReached\"",
        "\"terminalSpawnFailed\"",
        "\"internalError\"",
    ] {
        assert!(
            first.contains(needle),
            "generated TS bindings missing expected fragment: {needle}"
        );
    }
}
