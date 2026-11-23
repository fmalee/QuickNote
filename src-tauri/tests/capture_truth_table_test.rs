use std::collections::BTreeMap;

use quicknote_lib::domain::capture::{
    resolve_target, ResolveDocCandidate, ResolveTargetContext, TargetKind, TargetResolveInput,
};
use quicknote_lib::domain::settings::WorkspaceConfig;
use quicknote_lib::domain::ui_state::{CaptureMode, LastActiveDocState, UiState};

const NOW_MS: i64 = 1_732_400_000_000;
const HOUR_MS: i64 = 3_600_000;
const TODAY: &str = "2026-11-24";

fn sample_workspaces() -> BTreeMap<String, WorkspaceConfig> {
    let mut map = BTreeMap::new();
    map.insert(
        "alpha".to_string(),
        WorkspaceConfig {
            name: "Alpha".to_string(),
            key: "alpha".to_string(),
            root: "/tmp/alpha".to_string(),
            open_dir: "/tmp/alpha".to_string(),
            code_workspace: None,
            terminal_dir: None,
            default_editor: "vscode".to_string(),
            default_entry_doc: Some("projects/alpha-entry.md".to_string()),
        },
    );
    map.insert(
        "beta".to_string(),
        WorkspaceConfig {
            name: "Beta".to_string(),
            key: "beta".to_string(),
            root: "/tmp/beta".to_string(),
            open_dir: "/tmp/beta".to_string(),
            code_workspace: None,
            terminal_dir: None,
            default_editor: "vscode".to_string(),
            default_entry_doc: None,
        },
    );
    map
}

fn sample_docs() -> Vec<ResolveDocCandidate> {
    vec![
        ResolveDocCandidate {
            key: "k-inbox".to_string(),
            rel_path: "inbox.md".to_string(),
            title: "收件箱".to_string(),
            project: None,
            has_ideas_section: None,
            has_todo_section: None,
        },
        ResolveDocCandidate {
            key: "k-alpha-entry".to_string(),
            rel_path: "projects/alpha-entry.md".to_string(),
            title: "Alpha 入口".to_string(),
            project: Some("alpha".to_string()),
            has_ideas_section: Some(true),
            has_todo_section: Some(false),
        },
        ResolveDocCandidate {
            key: "k-alpha-note".to_string(),
            rel_path: "projects/alpha-note.md".to_string(),
            title: "Alpha 笔记".to_string(),
            project: Some("alpha".to_string()),
            has_ideas_section: Some(true),
            has_todo_section: Some(true),
        },
        ResolveDocCandidate {
            key: "k-beta-note".to_string(),
            rel_path: "projects/beta-note.md".to_string(),
            title: "Beta 笔记".to_string(),
            project: Some("beta".to_string()),
            has_ideas_section: Some(false),
            has_todo_section: Some(true),
        },
    ]
}

struct TruthCase {
    name: &'static str,
    library_configured: bool,
    input: TargetResolveInput,
    last_active: Option<LastActiveDocState>,
    workspace_modes: BTreeMap<String, CaptureMode>,
    expected_scenario: &'static str,
    expected_kind: TargetKind,
    expected_rel_path: Option<&'static str>,
    expected_cross_ws: bool,
    expected_mode: CaptureMode,
    expected_chip_label: &'static str,
}

#[test]
fn resolve_target_covers_s1_through_s8_truth_table() {
    let workspaces = sample_workspaces();
    let docs = sample_docs();

    let mut alpha_todo_mode = BTreeMap::new();
    alpha_todo_mode.insert("alpha".to_string(), CaptureMode::Todo);

    let cases = vec![
        TruthCase {
            name: "S1 unconfigured library",
            library_configured: false,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: Some("k-alpha-note".to_string()),
                selected_workspace_key: Some("alpha".to_string()),
                is_inbox_view: false,
            },
            last_active: None,
            workspace_modes: BTreeMap::new(),
            expected_scenario: "S1",
            expected_kind: TargetKind::Unconfigured,
            expected_rel_path: None,
            expected_cross_ws: false,
            expected_mode: CaptureMode::List,
            expected_chip_label: "请先选择或创建笔记库",
        },
        TruthCase {
            name: "S2 main visible with active note and workspace mode memory",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: Some("k-alpha-note".to_string()),
                selected_workspace_key: Some("beta".to_string()),
                is_inbox_view: false,
            },
            last_active: None,
            workspace_modes: alpha_todo_mode.clone(),
            expected_scenario: "S2",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-note.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::Todo,
            expected_chip_label: "Alpha 笔记 › 待办 · Alpha",
        },
        TruthCase {
            name: "S2 main visible with active inbox forces list mode",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: Some("inbox.md".to_string()),
                selected_workspace_key: None,
                is_inbox_view: true,
            },
            last_active: None,
            workspace_modes: alpha_todo_mode.clone(),
            expected_scenario: "S2",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::List,
            expected_chip_label: "收件箱 › 2026-11-24",
        },
        TruthCase {
            name: "S3 main visible no active doc reuses same-workspace last_active at exact 24h boundary",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
                selected_workspace_key: Some("alpha".to_string()),
                is_inbox_view: false,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - 24 * HOUR_MS,
            }),
            workspace_modes: alpha_todo_mode.clone(),
            expected_scenario: "S3",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-note.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::Todo,
            expected_chip_label: "Alpha 笔记 › 待办 · Alpha",
        },
        TruthCase {
            name: "S4 main visible expired TTL falls back to workspace defaultEntryDoc",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
                selected_workspace_key: Some("alpha".to_string()),
                is_inbox_view: false,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - 24 * HOUR_MS - 1,
            }),
            workspace_modes: BTreeMap::new(),
            expected_scenario: "S4",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-entry.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::List,
            expected_chip_label: "Alpha 入口 › 想法 · Alpha",
        },
        TruthCase {
            name: "S4 main visible cross-workspace last_active without defaultEntryDoc falls back to inbox with warning chip",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
                selected_workspace_key: Some("beta".to_string()),
                is_inbox_view: false,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            workspace_modes: alpha_todo_mode.clone(),
            expected_scenario: "S4",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_cross_ws: true,
            expected_mode: CaptureMode::List,
            expected_chip_label: "收件箱 · 跨工作区",
        },
        TruthCase {
            name: "S5 main visible no workspace in inbox view targets inbox",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
                selected_workspace_key: None,
                is_inbox_view: true,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            workspace_modes: BTreeMap::new(),
            expected_scenario: "S5",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::List,
            expected_chip_label: "收件箱 › 2026-11-24",
        },
        TruthCase {
            name: "S5 main visible no workspace not inbox view reuses valid last_active",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
                selected_workspace_key: None,
                is_inbox_view: false,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/beta-note.md".to_string(),
                workspace_key: Some("beta".to_string()),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            workspace_modes: BTreeMap::new(),
            expected_scenario: "S5",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/beta-note.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::List,
            expected_chip_label: "Beta 笔记 › 将追加到文末 · Beta",
        },
        TruthCase {
            name: "S6 hidden main ignores active_doc_key and uses same-workspace last_active",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: false,
                active_doc_key: Some("k-beta-note".to_string()),
                selected_workspace_key: Some("alpha".to_string()),
                is_inbox_view: false,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            workspace_modes: alpha_todo_mode.clone(),
            expected_scenario: "S6",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-note.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::Todo,
            expected_chip_label: "Alpha 笔记 › 待办 · Alpha",
        },
        TruthCase {
            name: "S7 hidden main cross-workspace last_active falls back to inbox with cross-workspace label",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: false,
                active_doc_key: Some("k-beta-note".to_string()),
                selected_workspace_key: Some("beta".to_string()),
                is_inbox_view: false,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            workspace_modes: BTreeMap::new(),
            expected_scenario: "S7",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_cross_ws: true,
            expected_mode: CaptureMode::List,
            expected_chip_label: "收件箱 · 跨工作区",
        },
        TruthCase {
            name: "S8 hidden main no workspace ignores active_doc_key and uses valid last_active",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: false,
                active_doc_key: Some("k-beta-note".to_string()),
                selected_workspace_key: None,
                is_inbox_view: true,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                workspace_key: Some("alpha".to_string()),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            workspace_modes: alpha_todo_mode,
            expected_scenario: "S8",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-note.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::Todo,
            expected_chip_label: "Alpha 笔记 › 待办 · Alpha",
        },
        TruthCase {
            name: "S8 hidden main no workspace without valid last_active falls back to inbox",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: false,
                active_doc_key: Some("k-beta-note".to_string()),
                selected_workspace_key: None,
                is_inbox_view: false,
            },
            last_active: None,
            workspace_modes: BTreeMap::new(),
            expected_scenario: "S8",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_cross_ws: false,
            expected_mode: CaptureMode::List,
            expected_chip_label: "收件箱 › 2026-11-24",
        },
    ];

    for case in cases {
        let ui_state = UiState {
            last_active_doc: case.last_active,
            workspace_modes: case.workspace_modes,
            ..Default::default()
        };

        let ctx = ResolveTargetContext {
            library_configured: case.library_configured,
            workspaces: &workspaces,
            ui_state: &ui_state,
            docs: &docs,
            last_active_ttl_hours: 24,
            auto_create_missing_section: false,
            now_ms: NOW_MS,
            today_date: TODAY,
        };

        let resolved = resolve_target(&ctx, &case.input);
        assert_eq!(
            resolved.scenario, case.expected_scenario,
            "case={}",
            case.name
        );
        assert_eq!(resolved.kind, case.expected_kind, "case={}", case.name);
        assert_eq!(
            resolved.target_rel_path.as_deref(),
            case.expected_rel_path,
            "case={}",
            case.name
        );
        assert_eq!(
            resolved.is_cross_workspace_fallback, case.expected_cross_ws,
            "case={}",
            case.name
        );
        assert_eq!(
            resolved.recommended_mode, case.expected_mode,
            "case={}",
            case.name
        );
        assert_eq!(
            resolved.chip_label, case.expected_chip_label,
            "case={}",
            case.name
        );
    }
}
