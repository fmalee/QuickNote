use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use quicknote_lib::application::capture::{CaptureManager, MAX_UNDO_BODY_BYTES};
use quicknote_lib::application::index::index_rebuild;
use quicknote_lib::application::library::set_library_root;
use quicknote_lib::application::notes::{note_read, note_write, search_notes_and_files};
use quicknote_lib::application::persistence::PersistenceManager;
use quicknote_lib::domain::capture::AppendNoteArgs;
use quicknote_lib::domain::note::{ReadNoteArgs, SearchQueryArgs, WriteNoteArgs};
use quicknote_lib::domain::settings::SetRootArgs;
use quicknote_lib::domain::ui_state::CaptureMode;

static PERF_DIR_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let seq = PERF_DIR_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("quicknote_m4_perf_{}_{}", std::process::id(), seq));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
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

fn median_ms(samples: &mut [f64]) -> f64 {
    samples.sort_by(|a, b| a.total_cmp(b));
    let mid = samples.len() / 2;
    if samples.len() % 2 == 1 {
        samples[mid]
    } else {
        (samples[mid - 1] + samples[mid]) / 2.0
    }
}

fn measure_search_median_ms(pm: &PersistenceManager, query: &str, runs: usize) -> (f64, usize) {
    // 预热 1 次，排除冷缓存抖动
    let warmup = search_notes_and_files(
        pm,
        SearchQueryArgs {
            query: query.to_string(),
            limit: Some(30),
            request_id: 1,
        },
    )
    .expect("warmup search");
    let hit_count = warmup.items.len();

    let mut samples = Vec::with_capacity(runs);
    for idx in 0..runs {
        let start = Instant::now();
        let res = search_notes_and_files(
            pm,
            SearchQueryArgs {
                query: query.to_string(),
                limit: Some(30),
                request_id: (idx + 2) as u64,
            },
        )
        .expect("benchmark search");
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        assert!(!res.items.is_empty(), "query `{query}` should return hits");
        samples.push(elapsed_ms);
    }

    (median_ms(&mut samples), hit_count)
}

fn generate_10k_vault(vault_dir: &Path) -> PathBuf {
    fs::create_dir_all(vault_dir).expect("create vault dir");
    fs::write(
        vault_dir.join("inbox.md"),
        "---\nid: 01J00000000000000000000000\ntitle: 收件箱\ntype: inbox\n---\n# Inbox\n",
    )
    .expect("write inbox.md");

    let subdirs = 20;
    for d in 0..subdirs {
        fs::create_dir_all(vault_dir.join(format!("topic_{d:02}"))).expect("create subdir");
    }

    let mut target_note_path = PathBuf::new();

    for i in 0..10_000usize {
        let subdir = format!("topic_{:02}", i % subdirs);
        let file_name = format!("perf-note-{i:05}.md");
        let path = vault_dir.join(&subdir).join(&file_name);
        if i == 420 {
            target_note_path = path.clone();
        }

        let cn_topic = match i % 5 {
            0 => "分布式架构与服务治理实践",
            1 => "网关配置与高可用路由策略",
            2 => "向量检索与混合分词调优",
            3 => "本地知识库增量索引同步",
            _ => "桌面端内存控制与响应延迟",
        };
        let en_keyword = match i % 4 {
            0 => "kubernetes deployment rollout",
            1 => "sqlite fts5 tokenizer benchmark",
            2 => "prometheus latency histogram",
            _ => "tauri ipc channel streaming",
        };
        let mixed_marker = if i % 25 == 0 {
            "微服务 latency 观测指标与链路追踪"
        } else {
            "常规技术记录与日常复盘"
        };

        let content = format!(
            "---\n\
             id: 01JPERF{:019}\n\
             title: 压测笔记 {i:05} - {cn_topic}\n\
             tags:\n\
               - perf\n\
               - topic-{}\n\
             project: quicknote-bench-{}\n\
             ---\n\
             # 压测笔记 {i:05}\n\n\
             本文记录 {cn_topic}，涵盖 {mixed_marker}。\n\
             关键技术词：{en_keyword}。\n\n\
             ```bash\n\
             kubectl get pods -n bench-{} && echo \"ok-{i}\"\n\
             ```\n",
            i,
            i % 10,
            i % 8,
            i % 5
        );
        fs::write(&path, content).expect("write generated note");
    }

    target_note_path
}

#[test]
#[ignore = "10k vault / 6MB doc perf benchmark; run with -- --ignored"]
fn m4_performance_budget_10k_vault_and_6mb_doc() {
    let tmp = TempDir::new();
    let app_data = tmp.path().join("app_data");
    let vault_dir = tmp.path().join("vault");

    let target_note_path = generate_10k_vault(&vault_dir);

    let pm = PersistenceManager::open(app_data).expect("open PersistenceManager");
    set_library_root(
        &pm,
        SetRootArgs {
            root: vault_dir.to_string_lossy().to_string(),
            create_if_missing: false,
        },
    )
    .expect("set library root");

    // 1. 万篇文档全量索引构建
    let rebuild_start = Instant::now();
    let status = index_rebuild(&pm, None).expect("rebuild 10k index");
    let index_10k_ms = rebuild_start.elapsed().as_secs_f64() * 1000.0;
    assert!(
        status.notes_count >= 10_000,
        "expected >= 10,000 indexed notes, got {}",
        status.notes_count
    );
    assert!(!status.fts_degraded, "FTS5 index must not be degraded");

    // 2. 万篇文档 FTS5 全文检索（纯中文、纯英文、中英混合）
    let (fts5_10k_cn_ms, cn_hits) = measure_search_median_ms(&pm, "分布式架构", 7);
    let (fts5_10k_en_ms, en_hits) = measure_search_median_ms(&pm, "kubernetes", 7);
    let (fts5_10k_mixed_ms, mixed_hits) = measure_search_median_ms(&pm, "微服务 latency", 7);

    // 3. 本地索引快速打开查询（精确标题关键词）
    let (search_local_ms, local_hits) = measure_search_median_ms(&pm, "压测笔记 00420", 7);

    // 4. 普通笔记原子保存（note_write）
    let abs_str = target_note_path.to_string_lossy().to_string();
    let initial_doc = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: abs_str.clone(),
        },
    )
    .expect("read target note");

    let warmup_write = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: abs_str.clone(),
            body: format!("{}\n- 预热写入\n", initial_doc.body),
            base_body_hash: initial_doc.body_hash,
        },
    )
    .expect("warmup note_write");

    let mut current_hash = warmup_write.body_hash;
    let mut current_body = format!("{}\n- 预热写入\n", initial_doc.body);
    let mut write_samples = Vec::with_capacity(5);
    for round in 0..5 {
        current_body.push_str(&format!("- 原子保存压测第 {round} 轮\n"));
        let start = Instant::now();
        let written = note_write(
            &pm,
            WriteNoteArgs {
                abs_path: abs_str.clone(),
                body: current_body.clone(),
                base_body_hash: current_hash,
            },
        )
        .expect("benchmark note_write");
        write_samples.push(start.elapsed().as_secs_f64() * 1000.0);
        current_hash = written.body_hash;
    }
    let note_write_ms = median_ms(&mut write_samples);

    println!(
        "[perf] index_10k_ms={index_10k_ms:.2} notes={} fts5_10k_cn_ms={fts5_10k_cn_ms:.2} (hits={cn_hits}) \
         fts5_10k_en_ms={fts5_10k_en_ms:.2} (hits={en_hits}) fts5_10k_mixed_ms={fts5_10k_mixed_ms:.2} (hits={mixed_hits}) \
         search_local_ms={search_local_ms:.2} (hits={local_hits}) note_write_ms={note_write_ms:.2}",
        status.notes_count
    );

    #[cfg(not(debug_assertions))]
    {
        assert!(
            fts5_10k_cn_ms < 50.0,
            "release FTS5 CN budget < 50ms, got {fts5_10k_cn_ms:.2}ms"
        );
        assert!(
            fts5_10k_en_ms < 50.0,
            "release FTS5 EN budget < 50ms, got {fts5_10k_en_ms:.2}ms"
        );
        assert!(
            fts5_10k_mixed_ms < 50.0,
            "release FTS5 mixed budget < 50ms, got {fts5_10k_mixed_ms:.2}ms"
        );
        assert!(
            search_local_ms < 30.0,
            "release local quick-open search budget < 30ms, got {search_local_ms:.2}ms"
        );
        assert!(
            note_write_ms < 50.0,
            "release note_write budget < 50ms, got {note_write_ms:.2}ms"
        );
    }

    #[cfg(debug_assertions)]
    {
        assert!(
            fts5_10k_cn_ms < 200.0,
            "debug FTS5 CN budget < 200ms, got {fts5_10k_cn_ms:.2}ms"
        );
        assert!(
            fts5_10k_en_ms < 200.0,
            "debug FTS5 EN budget < 200ms, got {fts5_10k_en_ms:.2}ms"
        );
        assert!(
            fts5_10k_mixed_ms < 200.0,
            "debug FTS5 mixed budget < 200ms, got {fts5_10k_mixed_ms:.2}ms"
        );
        assert!(
            search_local_ms < 120.0,
            "debug local quick-open search budget < 120ms, got {search_local_ms:.2}ms"
        );
        assert!(
            note_write_ms < 200.0,
            "debug note_write budget < 200ms, got {note_write_ms:.2}ms"
        );
    }

    // 5. 6MB 超大文档边界验收（note_read / note_write 与 >5MB note_append undo_available=false）
    let huge_rel = "huge-6mb.md";
    let huge_path = vault_dir.join(huge_rel);
    let chunk = "QuickNote 6MB 超大文档性能与撤销保护边界测试行——包含中文与 ASCII 内容。\n";
    let target_bytes = 6 * 1024 * 1024 + 4096;
    let mut huge_body = String::with_capacity(target_bytes + 256);
    while huge_body.len() < target_bytes {
        huge_body.push_str(chunk);
    }
    assert!(huge_body.len() > MAX_UNDO_BODY_BYTES);
    let huge_raw =
        format!("---\nid: 01JHUGE6MB0000000000000000\ntitle: 6MB 超大文档\n---\n{huge_body}");
    fs::write(&huge_path, &huge_raw).expect("write 6MB note");

    let huge_abs = huge_path.to_string_lossy().to_string();
    let read_huge = note_read(
        &pm,
        ReadNoteArgs {
            abs_path: huge_abs.clone(),
        },
    )
    .expect("read 6MB note");
    assert!(read_huge.size_bytes > 6 * 1024 * 1024);

    let updated_huge_body = format!("{}\n# Tail Marker\n", read_huge.body);
    let written_huge = note_write(
        &pm,
        WriteNoteArgs {
            abs_path: huge_abs,
            body: updated_huge_body,
            base_body_hash: read_huge.body_hash,
        },
    )
    .expect("write 6MB note");
    assert!(!written_huge.body_hash.is_empty());

    let capture = CaptureManager::new();
    let append_res = capture
        .note_append(
            &pm,
            AppendNoteArgs {
                target_rel_path: huge_rel.to_string(),
                content: "6MB 追加内容验证".to_string(),
                mode: CaptureMode::Prose,
                workspace_key: None,
            },
            None,
        )
        .expect("append to 6MB note");
    assert!(
        !append_res.undo_available,
        "notes > 5MB must set undo_available = false"
    );
}
