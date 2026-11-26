use quicknote_lib::domain::capture::{
    resolve_target, ResolveDocCandidate, ResolveTargetContext, TargetKind, TargetResolveInput,
};
use quicknote_lib::domain::ui_state::{CaptureMode, LastActiveDocState, UiState};

const NOW_MS: i64 = 1_732_400_000_000;
const HOUR_MS: i64 = 3_600_000;
const TODAY: &str = "2026-11-24";

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
    expected_scenario: &'static str,
    expected_kind: TargetKind,
    expected_rel_path: Option<&'static str>,
    expected_mode: CaptureMode,
    expected_section_hint: Option<&'static str>,
}

#[test]
fn capture_target_truth_table_s1_through_s4() {
    let docs = sample_docs();

    let cases = vec![
        // S1: libraryRoot == None -> unconfigured
        TruthCase {
            name: "S1: library unconfigured disables submit",
            library_configured: false,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: Some("k-alpha-note".to_string()),
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            expected_scenario: "S1",
            expected_kind: TargetKind::Unconfigured,
            expected_rel_path: None,
            expected_mode: CaptureMode::List,
            expected_section_hint: None,
        },
        // S2: mainVisible && activeDoc != None -> target = activeDoc (regular note)
        TruthCase {
            name: "S2: main visible with active regular note targets active note",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: Some("k-alpha-note".to_string()),
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/beta-note.md".to_string(),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            expected_scenario: "S2",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-note.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some("想法"),
        },
        // S2: mainVisible && activeDoc == inbox.md -> kind = inbox
        TruthCase {
            name: "S2: main visible with active inbox targets inbox",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: Some("inbox.md".to_string()),
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                touched_at_ms: NOW_MS - HOUR_MS,
            }),
            expected_scenario: "S2",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some(TODAY),
        },
        // S3: mainVisible && activeDoc == None && lastActiveDoc in 24h -> lastActiveDoc
        TruthCase {
            name: "S3: main visible without active doc picks recent lastActiveDoc",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/beta-note.md".to_string(),
                touched_at_ms: NOW_MS - 2 * HOUR_MS,
            }),
            expected_scenario: "S3",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/beta-note.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some("将追加到文末"),
        },
        // S3: mainVisible && activeDoc == None && lastActiveDoc expired -> inbox.md
        TruthCase {
            name:
                "S3: main visible without active doc falls back to inbox when lastActiveDoc expired",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                touched_at_ms: NOW_MS - 25 * HOUR_MS,
            }),
            expected_scenario: "S3",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some(TODAY),
        },
        // S3: mainVisible && activeDoc == None && no lastActiveDoc -> inbox.md
        TruthCase {
            name: "S3: main visible without active doc or lastActiveDoc falls back to inbox",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: true,
                active_doc_key: None,
            },
            last_active: None,
            expected_scenario: "S3",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some(TODAY),
        },
        // S4: !mainVisible && lastActiveDoc in 24h -> ignores hidden activeDoc and targets lastActiveDoc
        TruthCase {
            name: "S4: hidden main ignores activeDoc and targets recent lastActiveDoc",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: false,
                active_doc_key: Some("k-beta-note".to_string()),
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                touched_at_ms: NOW_MS - 3 * HOUR_MS,
            }),
            expected_scenario: "S4",
            expected_kind: TargetKind::Note,
            expected_rel_path: Some("projects/alpha-note.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some("想法"),
        },
        // S4: !mainVisible && lastActiveDoc missing or expired -> inbox.md
        TruthCase {
            name: "S4: hidden main without valid lastActiveDoc falls back to inbox",
            library_configured: true,
            input: TargetResolveInput {
                main_visible: false,
                active_doc_key: Some("k-alpha-note".to_string()),
            },
            last_active: Some(LastActiveDocState {
                rel_path: "projects/alpha-note.md".to_string(),
                touched_at_ms: NOW_MS - 30 * HOUR_MS,
            }),
            expected_scenario: "S4",
            expected_kind: TargetKind::Inbox,
            expected_rel_path: Some("inbox.md"),
            expected_mode: CaptureMode::List,
            expected_section_hint: Some(TODAY),
        },
    ];

    for case in cases {
        let ui_state = UiState {
            last_active_doc: case.last_active,
            ..UiState::default()
        };
        let ctx = ResolveTargetContext {
            library_configured: case.library_configured,
            ui_state: &ui_state,
            docs: &docs,
            last_active_ttl_hours: 24,
            auto_create_missing_section: false,
            now_ms: NOW_MS,
            today_date: TODAY,
        };
        let actual = resolve_target(&ctx, &case.input);
        assert_eq!(
            actual.scenario, case.expected_scenario,
            "scenario mismatch in case: {}",
            case.name
        );
        assert_eq!(
            actual.kind, case.expected_kind,
            "kind mismatch in case: {}",
            case.name
        );
        assert_eq!(
            actual.target_rel_path.as_deref(),
            case.expected_rel_path,
            "target_rel_path mismatch in case: {}",
            case.name
        );
        assert_eq!(
            actual.recommended_mode, case.expected_mode,
            "recommended_mode mismatch in case: {}",
            case.name
        );
        assert_eq!(
            actual.section_hint.as_deref(),
            case.expected_section_hint,
            "section_hint mismatch in case: {}",
            case.name
        );
    }
}
