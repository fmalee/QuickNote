use std::fs;
use std::path::{Path, PathBuf};

use quicknote_lib::application::index::index_rebuild;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{
    commands_search, note_delete, note_read, note_update_meta, note_write, notes_get_related,
    notes_list, tags_batch_rename,
};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::command_book::{filter_and_rank_commands, parse_command_book};
use quicknote_lib::domain::error::ErrorCode;
use quicknote_lib::domain::markdown::extract_relative_note_links;
use quicknote_lib::domain::note::{
    compute_body_hash, parse_note, NoteMetaPatch, NoteType, NoteUpdateMetaArgs, NotesFilterArgs,
    ReadNoteArgs, RelPathArgs, RelatedReason, TagRenameArgs, WriteNoteArgs,
};
use quicknote_lib::domain::settings::SetRootArgs;

static TEST_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, std::io::Error> {
        let seq = TEST_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("quicknote_m3_test_{}_{}", std::process::id(), seq));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn fixture_vault_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create_dir_all dst");
    for entry in fs::read_dir(src).expect("read_dir src") {
        let entry = entry.expect("dir entry");
        let ty = entry.file_type().expect("file_type");
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path);
        } else if ty.is_file() {
            fs::copy(entry.path(), &dst_path).expect("copy fixture file");
        }
    }
}

fn setup_pm(app_dir: &Path, library_dir: &Path) -> PersistenceManager {
    let pm = PersistenceManager::open(app_dir.to_path_buf()).expect("init PersistenceManager");
    set_library_root(
        &pm,
        SetRootArgs {
            root: library_dir.to_string_lossy().to_string(),
            create_if_missing: true,
        },
    )
    .expect("set_library_root");
    pm
}

#[test]
fn parses_fixture_command_book_with_exact_lines_and_metadata() {
    let fixture_path = fixture_vault_root().join("commands/常用命令.md");
    let raw = fs::read_to_string(&fixture_path).expect("read 常用命令.md fixture");
    let parsed = parse_note(&raw, "常用命令");
    assert_eq!(parsed.meta.doc_type, NoteType::CommandBook);

    let entries = parse_command_book("commands/常用命令.md", &parsed.body);
    assert_eq!(entries.len(), 3);

    let e0 = &entries[0];
    assert_eq!(e0.section, "Git");
    assert_eq!(e0.title, "撤销最近一次提交，但保留改动");
    assert_eq!(e0.description, "用于改错 commit message 或补文件。");
    assert_eq!(e0.code, "git reset --soft HEAD~1");
    assert_eq!(e0.language.as_deref(), Some("bash"));
    assert_eq!(e0.keywords, vec!["撤销", "commit", "回退", "reset"]);
    assert!(!e0.is_danger);
    assert_eq!(e0.line, 4);

    let e1 = &entries[1];
    assert_eq!(e1.section, "Git");
    assert_eq!(e1.title, "强制清理未跟踪文件与目录");
    assert_eq!(e1.description, "");
    assert_eq!(e1.code, "git clean -fd");
    assert_eq!(e1.language.as_deref(), Some("bash"));
    assert!(e1.is_danger);
    assert_eq!(e1.line, 13);

    let e2 = &entries[2];
    assert_eq!(e2.section, "Nginx");
    assert_eq!(e2.title, "重载 nginx.conf 配置而不中断服务");
    assert_eq!(e2.description, "");
    assert_eq!(e2.code, "nginx -s reload");
    assert_eq!(e2.language.as_deref(), Some("bash"));
    assert!(!e2.is_danger);
    assert_eq!(e2.line, 22);

    // Pure ranking check
    let ranked = filter_and_rank_commands(&entries, "回退");
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].id, e0.id);
}

#[test]
fn index_rebuild_and_commands_search_sync_on_write_and_delete() {
    let tmp = TempDir::new().expect("tempdir");
    let app_dir = tmp.path().join("app");
    let vault_dir = tmp.path().join("vault");
    copy_dir_recursive(&fixture_vault_root(), &vault_dir);

    let pm = setup_pm(&app_dir, &vault_dir);
    index_rebuild(&pm, None).expect("index_rebuild");

    let all = commands_search(
        &pm,
        quicknote_lib::domain::command_book::CommandSearchArgs {
            query: String::new(),
        },
    )
    .expect("commands_search empty");
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].line, 4);
    assert_eq!(all[1].line, 13);
    assert_eq!(all[2].line, 22);

    let by_kw = commands_search(
        &pm,
        quicknote_lib::domain::command_book::CommandSearchArgs {
            query: "回退".to_string(),
        },
    )
    .expect("commands_search 回退");
    assert_eq!(by_kw.len(), 1);
    assert_eq!(by_kw[0].title, "撤销最近一次提交，但保留改动");

    let by_code = commands_search(
        &pm,
        quicknote_lib::domain::command_book::CommandSearchArgs {
            query: "nginx".to_string(),
        },
    )
    .expect("commands_search nginx");
    assert_eq!(by_code.len(), 1);
    assert_eq!(by_code[0].code, "nginx -s reload");

    // Incremental update via note_write
    let cmd_abs = vault_dir.join("commands/常用命令.md");
    let doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: cmd_abs.to_string_lossy().to_string(),
        },
    )
    .expect("note_read command book");
    let new_body = format!(
        "{}\n\n### 查看监听端口\n\n```bash\nlsof -i :8080\n```\n<!-- keywords: 端口 port -->\n",
        doc.body
    );
    note_write(
        &pm,
        WriteNoteArgs {
            abs_path: cmd_abs.to_string_lossy().to_string(),
            body: new_body,
            base_body_hash: doc.body_hash,
        },
    )
    .expect("note_write command book");

    let after_write = commands_search(
        &pm,
        quicknote_lib::domain::command_book::CommandSearchArgs {
            query: "port".to_string(),
        },
    )
    .expect("commands_search port");
    assert_eq!(after_write.len(), 1);
    assert_eq!(after_write[0].title, "查看监听端口");

    // Incremental delete via note_delete
    note_delete(
        &pm,
        RelPathArgs {
            rel_path: "commands/常用命令.md".to_string(),
        },
    )
    .expect("note_delete command book");
    let after_delete = commands_search(
        &pm,
        quicknote_lib::domain::command_book::CommandSearchArgs {
            query: String::new(),
        },
    )
    .expect("commands_search after delete");
    assert!(after_delete.is_empty());
}

#[test]
fn extracts_relative_note_links_ignoring_code_external_and_escaping() {
    let body = r#"
Here is a [valid link](./child.md#heading?foo=1) and [parent](../root.markdown "Title").
Ignore [self](./sub.md), [external](https://example.com/a.md), [mail](mailto:a@b.com),
[abs](/etc/passwd.md), [anchor](#sec), [escape](../../../outside.md), ![img](./pic.png).

Inline code `[ignored](./nope.md)` and ``[also](./nope2.md)``.

```markdown
[fenced](./fenced.md)
```

[ref1]: <../other/target.md#part> "Ref Title"
[ref2]: ./child.md
"#;

    let links = extract_relative_note_links("notes/sub.md", body);
    assert_eq!(
        links,
        vec![
            "notes/child.md".to_string(),
            "other/target.md".to_string(),
            "root.markdown".to_string(),
        ]
    );
}

#[test]
fn notes_get_related_scores_weights_order_and_limit_20() {
    let tmp = TempDir::new().expect("tempdir");
    let app_dir = tmp.path().join("app");
    let vault_dir = tmp.path().join("vault");
    fs::create_dir_all(vault_dir.join("notes")).expect("mkdir notes");
    fs::create_dir_all(vault_dir.join("_templates")).expect("mkdir templates");

    fs::write(
        vault_dir.join("notes/target.md"),
        "---\ntitle: Target\ntags: [rust, tauri]\nproject: quicknote\n---\nBody\n",
    )
    .expect("write target");

    // Backlink (+100) + 1 shared tag (+15) = 115
    fs::write(
        vault_dir.join("notes/backlinker.md"),
        "---\ntitle: Backlinker\ntags: [rust]\n---\nSee [target](./target.md)\n",
    )
    .expect("write backlinker");

    // Shared 2 tags (+30) + same project (+10) = 40
    fs::write(
        vault_dir.join("notes/tags_proj.md"),
        "---\ntitle: TagsProj\ntags: [rust, tauri]\nproject: quicknote\n---\nBody\n",
    )
    .expect("write tags_proj");

    // Template should be excluded even if matching tags/project/link
    fs::write(
        vault_dir.join("_templates/tpl.md"),
        "---\ntitle: Tpl\ntags: [rust, tauri]\nproject: quicknote\n---\n[t](../notes/target.md)\n",
    )
    .expect("write tpl");

    // Unrelated note (score 0)
    fs::write(
        vault_dir.join("notes/unrelated.md"),
        "---\ntitle: Unrelated\ntags: [other]\n---\nBody\n",
    )
    .expect("write unrelated");

    // 22 same-project-only notes (+10) to verify limit 20
    for i in 0..22 {
        fs::write(
            vault_dir.join(format!("notes/proj_{i:02}.md")),
            format!("---\ntitle: Proj {i:02}\nproject: quicknote\n---\nBody\n"),
        )
        .expect("write proj note");
    }

    let pm = setup_pm(&app_dir, &vault_dir);
    index_rebuild(&pm, None).expect("index_rebuild");

    let related = notes_get_related(
        &pm,
        RelPathArgs {
            rel_path: "notes/target.md".to_string(),
        },
    )
    .expect("notes_get_related");

    assert_eq!(related.len(), 20);
    assert_eq!(related[0].rel_path, "notes/backlinker.md");
    assert_eq!(related[0].reason, RelatedReason::Backlink);
    assert!((related[0].score - 115.0).abs() < 1e-6);

    assert_eq!(related[1].rel_path, "notes/tags_proj.md");
    assert_eq!(related[1].reason, RelatedReason::SharedTags);
    assert!((related[1].score - 40.0).abs() < 1e-6);

    assert_eq!(related[2].reason, RelatedReason::SameProject);
    assert!((related[2].score - 10.0).abs() < 1e-6);

    assert!(related.iter().all(|item| item.rel_path != "notes/target.md"
        && item.rel_path != "notes/unrelated.md"
        && !item.rel_path.starts_with("_templates/")));
}

#[test]
fn tags_batch_rename_merges_preserves_comments_and_skips_broken_yaml() {
    let tmp = TempDir::new().expect("tempdir");
    let app_dir = tmp.path().join("app");
    let vault_dir = tmp.path().join("vault");
    fs::create_dir_all(vault_dir.join("notes")).expect("mkdir notes");

    let a_path = vault_dir.join("notes/a.md");
    let a_content = "---\n# keep comment\ntitle: Note A\ntags:\n  - dev\n  - dev/git\n  - devops\n  - work\ncustom_unknown: keep_me\n---\nBody A\n";
    fs::write(&a_path, a_content).expect("write a.md");

    let broken_path = vault_dir.join("notes/broken.md");
    let broken_content = "---\ntitle: [broken\ntags: [dev]\n---\nBroken body\n";
    fs::write(&broken_path, broken_content).expect("write broken.md");

    let pm = setup_pm(&app_dir, &vault_dir);
    index_rebuild(&pm, None).expect("index_rebuild");

    // Validation error on identical or empty tag
    let err = tags_batch_rename(
        &pm,
        TagRenameArgs {
            old_tag: "dev".to_string(),
            new_tag: "#dev/".to_string(),
        },
    )
    .expect_err("same tag should fail");
    assert_eq!(err.code, ErrorCode::ValidationError);

    let res = tags_batch_rename(
        &pm,
        TagRenameArgs {
            old_tag: "dev".to_string(),
            new_tag: "work".to_string(),
        },
    )
    .expect("tags_batch_rename");
    assert_eq!(res.updated_files, 1);

    let a_after = fs::read_to_string(&a_path).expect("read a.md after");
    assert!(a_after.contains("# keep comment"));
    assert!(a_after.contains("custom_unknown: keep_me"));
    assert!(a_after.contains("Body A"));
    let parsed_a = parse_note(&a_after, "a");
    assert_eq!(parsed_a.meta.tags, vec!["work", "work/git", "devops"]);

    // Broken YAML file must remain untouched
    let broken_after = fs::read_to_string(&broken_path).expect("read broken.md after");
    assert_eq!(broken_after, broken_content);

    // Self-write guard recorded
    assert!(pm.should_suppress_external_modify(&a_path, &compute_body_hash(&parsed_a.body)));

    // Index refreshed
    let listed = notes_list(&pm, NotesFilterArgs::default()).expect("notes_list");
    let a_row = listed
        .iter()
        .find(|n| n.rel_path.as_deref() == Some("notes/a.md"))
        .expect("a.md indexed");
    assert_eq!(a_row.tags, vec!["work", "work/git", "devops"]);
}

#[test]
fn non_markdown_files_roundtrip_without_stripping_frontmatter_or_adding_ulid() {
    let tmp = TempDir::new().expect("tempdir");
    let app_dir = tmp.path().join("app");
    let vault_dir = tmp.path().join("vault");
    fs::create_dir_all(&vault_dir).expect("mkdir vault");
    let pm = setup_pm(&app_dir, &vault_dir);

    let ext_dir = tmp.path().join("external");
    fs::create_dir_all(&ext_dir).expect("mkdir external");

    let yaml_path = ext_dir.join("docker-compose.yaml");
    let yaml_raw = "---\nservices:\n  web:\n    image: nginx:latest\n";
    fs::write(&yaml_path, yaml_raw).expect("write yaml");

    let conf_path = ext_dir.join("nginx.conf");
    let conf_raw = "---\nserver {\n  listen 80;\n}\n";
    fs::write(&conf_path, conf_raw).expect("write conf");

    let yaml_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: yaml_path.to_string_lossy().to_string(),
        },
    )
    .expect("read yaml");
    assert_eq!(yaml_doc.body, yaml_raw);
    assert_eq!(yaml_doc.meta.title, "docker-compose.yaml");
    assert!(yaml_doc.meta.id.is_none());

    let updated_yaml = "---\nservices:\n  web:\n    image: nginx:1.27\n";
    let write_res = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: yaml_path.to_string_lossy().to_string(),
            body: updated_yaml.to_string(),
            base_body_hash: yaml_doc.body_hash.clone(),
        },
    )
    .expect("write yaml");
    assert!(write_res.updated_meta.is_none());
    assert_eq!(
        fs::read_to_string(&yaml_path).expect("read yaml back"),
        updated_yaml
    );

    // Stale hash triggers Conflict
    let conflict = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: yaml_path.to_string_lossy().to_string(),
            body: "stale".to_string(),
            base_body_hash: yaml_doc.body_hash,
        },
    )
    .expect_err("stale hash should conflict");
    assert_eq!(conflict.code, ErrorCode::Conflict);

    // .conf also preserved verbatim
    let conf_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: conf_path.to_string_lossy().to_string(),
        },
    )
    .expect("read conf");
    assert_eq!(conf_doc.body, conf_raw);

    // Non-Markdown files reject frontmatter metadata updates
    let meta_err = note_update_meta(
        &pm,
        NoteUpdateMetaArgs {
            abs_path: conf_path.to_string_lossy().to_string(),
            patch: NoteMetaPatch {
                title: Some("Hacked".to_string()),
                ..NoteMetaPatch::default()
            },
        },
    )
    .expect_err("non-markdown note_update_meta must fail");
    assert_eq!(meta_err.code, ErrorCode::ValidationError);
    assert_eq!(
        meta_err.message,
        "Only Markdown files support frontmatter metadata"
    );
    assert_eq!(
        fs::read_to_string(&conf_path).expect("read conf back"),
        conf_raw
    );
}
