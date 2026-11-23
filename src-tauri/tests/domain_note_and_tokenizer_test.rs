use std::fs;
use std::path::PathBuf;

use quicknote_lib::domain::frontmatter::{
    apply_yaml_patch, replace_frontmatter_yaml, YamlPatchError, YamlPatchOp, YamlPatchValue,
};
use quicknote_lib::domain::note::{
    compose_note, compute_body_hash, count_list_items, format_ulid, meta_patch_ops, parse_note,
    tag_matches, NoteMetaPatch, NoteType,
};
use quicknote_lib::domain::tokenizer::{
    build_fts_query, tokenize_mixed, tokenize_mixed_vec, tokenize_tags,
};

fn vault_fixture(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("vault")
        .join(rel);
    fs::read_to_string(&path).expect("fixture file should be readable")
}

#[test]
fn tokenizer_handles_mixed_cjk_and_ascii_without_ascii_bigrams() {
    let tokens = tokenize_mixed_vec("nginx.conf");
    assert_eq!(tokens, vec!["nginx.conf", "nginx", "conf"]);
    assert!(!tokens.iter().any(|t| t == "ng" || t == "gi" || t == "in"));

    let mixed = tokenize_mixed("在 macOS 上编辑 nginx.conf 配置文件");
    assert_eq!(
        mixed,
        "在 macos 上编 编辑 nginx.conf nginx conf 配置 置文 文件"
    );

    let cjk_single = tokenize_mixed_vec("我");
    assert_eq!(cjk_single, vec!["我"]);

    let cjk_multi = tokenize_mixed_vec("快速笔记");
    assert_eq!(cjk_multi, vec!["快速", "速笔", "笔记"]);

    let query = build_fts_query("nginx.conf 配置").expect("non-empty query");
    assert_eq!(query, "\"nginx.conf\" \"nginx\" \"conf\" \"配置\"*");
    assert_eq!(build_fts_query("   ---   "), None);

    let tags = tokenize_tags(&["dev/quicknote".to_string(), "idea".to_string()]);
    assert_eq!(tags, "dev/quicknote dev quicknote idea");
}

#[test]
fn inbox_fixture_parses_and_recomposes_losslessly() {
    let raw = vault_fixture("inbox.md");
    assert!(
        raw.lines().count() > 500,
        "inbox.md fixture must exceed 500 lines"
    );

    let parsed = parse_note(&raw, "inbox");
    assert!(parsed.yaml_patchable);
    assert_eq!(parsed.meta.meta_error, None);
    assert!(!parsed.body.starts_with("---\n"));
    assert!(count_list_items(&parsed.body) > 0);

    let recomposed = compose_note(parsed.block.as_deref(), &parsed.body);
    assert_eq!(recomposed, raw);
}

#[test]
fn missing_id_fixture_allows_ulid_backfill_and_body_hashing() {
    let raw = vault_fixture("missing-id.md");
    let parsed = parse_note(&raw, "missing-id");

    assert!(parsed.yaml_patchable);
    assert_eq!(parsed.meta.id, None);
    assert_eq!(parsed.meta.title, "旧库笔记");
    assert_eq!(parsed.meta.tags, vec!["legacy"]);
    assert_eq!(parsed.meta.meta_error, None);

    let hash1 = compute_body_hash(&parsed.body);
    let hash2 = compute_body_hash(&parsed.body);
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);
    assert!(hash1.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    assert_ne!(hash1, compute_body_hash("different body"));

    let ulid = format_ulid(1_732_380_000_000, 0x1234_5678_9ABC_DEF0_1122);
    assert_eq!(ulid.len(), 26);
    assert!(ulid.chars().all(|c| {
        matches!(c, '0'..='9' | 'A'..='H' | 'J'..='K' | 'M'..='N' | 'P'..='T' | 'V'..='Z')
    }));

    let yaml = parsed.yaml.as_deref().expect("yaml present");
    let patched_yaml = apply_yaml_patch(
        yaml,
        &[YamlPatchOp::set("id", YamlPatchValue::Str(ulid.clone()))],
    )
    .expect("id backfill patch succeeds");
    assert!(patched_yaml.starts_with(&format!("id: {ulid}")));

    let updated_raw = replace_frontmatter_yaml(&raw, &patched_yaml);
    let reparsed = parse_note(&updated_raw, "missing-id");
    assert_eq!(reparsed.meta.id.as_deref(), Some(ulid.as_str()));
    assert_eq!(reparsed.body, parsed.body);
}

#[test]
fn broken_yaml_fixture_degrades_gracefully_and_blocks_in_place_patch() {
    let raw = vault_fixture("broken-yaml.md");
    let parsed = parse_note(&raw, "broken-yaml");

    assert!(!parsed.yaml_patchable);
    assert!(parsed.meta.meta_error.is_some());
    assert_eq!(parsed.meta.title, "损坏样本");
    assert!(parsed.body.contains("正文依然可以正常打开与编辑"));

    let recomposed = compose_note(parsed.block.as_deref(), &parsed.body);
    assert_eq!(recomposed, raw);

    let yaml = parsed.yaml.as_deref().expect("raw yaml present");
    let patch_err = apply_yaml_patch(
        yaml,
        &[YamlPatchOp::set("pinned", YamlPatchValue::Bool(true))],
    )
    .expect_err("broken yaml must not be patched in place");
    assert!(matches!(patch_err, YamlPatchError::Unpatchable(_)));
}

#[test]
fn minimal_patch_preserves_unknown_keys_comments_and_key_order() {
    let raw = vault_fixture("unknown-keys-comments.md");
    let parsed = parse_note(&raw, "unknown-keys-comments");

    assert!(parsed.yaml_patchable);
    assert_eq!(parsed.meta.doc_type, NoteType::Doc);
    assert_eq!(
        parsed.meta.id.as_deref(),
        Some("01JDQN0TE5UNKNWNKEYS000001")
    );
    assert!(tag_matches("dev/quicknote", "dev"));
    assert!(tag_matches("dev/quicknote", "dev/quicknote"));
    assert!(!tag_matches("dev/quicknote", "de"));
    assert!(!tag_matches("dev/quicknote", "idea"));

    let patch = NoteMetaPatch {
        title: Some("更新后的标题".to_string()),
        pinned: Some(true),
        open_dir: Some(None),
        ..Default::default()
    };
    let ops = meta_patch_ops(&patch);
    let yaml = parsed.yaml.as_deref().expect("yaml present");
    let patched_yaml = apply_yaml_patch(yaml, &ops).expect("minimal patch succeeds");

    // 注释与未知字段原样保留
    assert!(patched_yaml.contains("# 顶部注释：此文件由旧工具生成，请勿删除"));
    assert!(patched_yaml.contains("# 行尾注释"));
    assert!(patched_yaml.contains("# 中间注释：以下为状态字段"));
    assert!(patched_yaml.contains("custom_field: 保留我"));
    assert!(patched_yaml.contains("x-extension:\n  nested: value\n  list:\n    - a\n    - b"));

    // 目标键更新或删除
    assert!(patched_yaml.contains("title: 更新后的标题"));
    assert!(patched_yaml.contains("pinned: true"));
    assert!(!patched_yaml.contains("open_dir:"));

    // 既有键顺序不变
    let pos_type = patched_yaml.find("type: doc").expect("type");
    let pos_title = patched_yaml.find("title: 更新后的标题").expect("title");
    let pos_custom = patched_yaml
        .find("custom_field: 保留我")
        .expect("custom_field");
    let pos_pinned = patched_yaml.find("pinned: true").expect("pinned");
    let pos_archived = patched_yaml.find("archived: false").expect("archived");
    let pos_id = patched_yaml
        .find("id: 01JDQN0TE5UNKNWNKEYS000001")
        .expect("id");
    assert!(pos_type < pos_title);
    assert!(pos_title < pos_custom);
    assert!(pos_custom < pos_pinned);
    assert!(pos_pinned < pos_archived);
    assert!(pos_archived < pos_id);

    let updated_raw = replace_frontmatter_yaml(&raw, &patched_yaml);
    let reparsed = parse_note(&updated_raw, "unknown-keys-comments");
    assert_eq!(reparsed.meta.title, "更新后的标题");
    assert!(reparsed.meta.pinned);
    assert_eq!(reparsed.meta.open_dir, None);
    assert_eq!(reparsed.body, parsed.body);
}
