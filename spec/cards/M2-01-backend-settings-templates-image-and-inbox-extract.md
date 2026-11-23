# M2-01：后端 M2 契约补全（`settings_update`、B.3 模板渲染、`note_save_image`、`inbox_extract_to_note` 与菜单桥接）

- 所属里程碑：M2
- 关联方案与契约：`v4` §3.8、§4.9–§4.11、§5.6.3、§5.11、§6 M2、附录 B.3；`C1.2`、`C1.3`、`C1.4`、`C2.1`、`C2.3`、`C3.1`–`C3.4`、`C4.2`、`C4.3`、`C6.2`、`C6.4`、`C8.5`、`C9.1`、`D-0006`、`D-0008`
- 前置依赖：`m1-baseline`（`dev`）
- 会话策略：独立新会话执行（零大文档读取，所有 Rust 结构体定义、模板全文、算法边界与 IPC 签名已 100% 内联于本卡）；完成后提交并结束会话

## 1. 目标范围（做什么 / 明确不做什么）
- 要做：
  - **1. 补全 `settings_update` IPC 命令与 `SettingsPatch` 体系（`C3.4`、`C4.2`、`C6.2`、`D-0008`）**：
    - 在 `src-tauri/src/domain/settings.rs` 新增以下派生 `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]` 与 `#[serde(rename_all = "camelCase")]` 的 Patch 结构体（所有字段均为 `#[serde(default)] Option<...>`）：
      - `HotkeySettingsPatch { quick_bar: Option<String>, quick_bar_backup: Option<String> }`
      - `QuickBarSettingsPatch { default_target: Option<String>, last_active_ttl_hours: Option<u32>, destroy_after_hidden_minutes: Option<u32>, auto_create_missing_section: Option<bool> }`
      - `MainWindowSettingsPatch { mode: Option<WindowMode>, auxiliary_width_percent: Option<u8> }`
      - `EditorSettingsPatch { theme: Option<EditorTheme>, font_size: Option<u16>, line_height: Option<f64> (标注 `#[specta(type = Option<specta_typescript::Number>)]`), max_width: Option<u32>, tab_size_prose: Option<u8>, tab_size_code: Option<u8>, auto_save_debounce_ms: Option<u32>, auto_save_max_wait_ms: Option<u32>, single_click_link: Option<bool> }`
      - `TerminalPanelSettingsPatch { visible_by_default: Option<bool>, width: Option<u32>, shell: Option<String>, idle_recycle_minutes: Option<u32>, max_concurrent_sessions: Option<u32> }`
      - `PanelsSettingsPatch { default: Option<PanelId>, width: Option<u32>, terminal: Option<TerminalPanelSettingsPatch> }`
      - `InboxSettingsPatch { bloat_threshold_lines: Option<u32> }`
      - `ExternalAppConfigPatch { name: Option<String>, bundle_id: Option<String> }`
      - `EditorsSettingsPatch { vscode: Option<ExternalAppConfigPatch>, sublime: Option<ExternalAppConfigPatch> }`
      - `SettingsPatch { library_root: Option<String>, hotkey: Option<HotkeySettingsPatch>, quick_bar: Option<QuickBarSettingsPatch>, main_window: Option<MainWindowSettingsPatch>, editor: Option<EditorSettingsPatch>, panels: Option<PanelsSettingsPatch>, inbox: Option<InboxSettingsPatch>, terminal: Option<ExternalAppConfigPatch>, editors: Option<EditorsSettingsPatch>, search_roots: Option<Vec<String>>, workspaces: Option<BTreeMap<String, WorkspaceConfig>> }`
    - 在 `domain::settings` 实现纯函数 `pub fn apply_settings_patch(base: &Settings, patch: &SettingsPatch) -> Result<Settings, AppError>`：
      - 逐字段合并非 `None` 项，并校验范围（越界返回 `ErrorCode::ValidationError`）：
        - `editor.font_size`: `10..=32`
        - `editor.line_height`: `1.0..=3.0`（且 `is_finite()`）
        - `editor.tab_size_prose` / `editor.tab_size_code`: `1..=8`
        - `editor.auto_save_debounce_ms`: `100..=60_000`
        - `editor.auto_save_max_wait_ms`: `>= editor.auto_save_debounce_ms` 且 `<= 120_000`
        - `inbox.bloat_threshold_lines`: `>= 10`
        - `main_window.auxiliary_width_percent`: `20..=80`
        - `panels.width` / `panels.terminal.width`: `>= 220`
        - `panels.terminal.max_concurrent_sessions`: `1..=10`
        - `quick_bar.last_active_ttl_hours`: `>= 1`
    - 在 `src-tauri/src/application/persistence.rs` 新增 `pub fn settings_update(&self, patch: SettingsPatch) -> Result<SettingsSnapshot, AppError>`：
      - **`D-0008` 只读保护铁律**：若当前 `self.settings_status` 为 `valid == false`，**绝不覆盖磁盘上损坏的 `settings.yaml`**，直接返回 `Err(AppError::new(ErrorCode::SettingsParseError, "settings.yaml 当前存在语法错误，已只读保护，请修复后再修改设置"))`；
      - 有效时：读取当前 `Settings`，调用 `apply_settings_patch`，经 `serialize_settings_yaml` + `atomic_write_str(&self.settings_path(), &yaml)` 原子落盘，同步更新内存 `settings`、`last_valid_settings` 与 `settings_status`（`valid: true, active_source: "file"`），返回 `SettingsSnapshot`。
    - 在 `src-tauri/src/commands/settings.rs` 新增 `#[tauri::command] pub fn settings_update(app: AppHandle, state: State<'_, Arc<PersistenceManager>>, args: SettingsPatch) -> Result<SettingsSnapshot, AppError>`，落盘成功后广播 `app.emit("settings:status", &snapshot.status)`（仅传状态元数据，不记内容）。
  - **2. 收口 M0→M2 挂起债务：默认模板与 `v4` 附录 B.3 逐字对齐 + 模板渲染引擎（`v4` §4.10、附录 B.3）**：
    - 新增纯领域模块 `src-tauri/src/domain/template.rs`（在 `domain/mod.rs` 导出），零 IO 依赖：
      - 精确定义三份内置默认模板常量（与 `v4` 附录 B.3 逐字一致，含结尾换行）：
        - `DEFAULT_TEMPLATE_NOTE`：
          ```markdown
          ---
          id: {{id}}
          title: {{title}}
          type: note
          tags: []
          created: "{{date}}T{{time}}"
          pinned: false
          archived: false
          ---
          # {{title}}

          ```
        - `DEFAULT_TEMPLATE_DOC`：
          ```markdown
          ---
          id: {{id}}
          title: {{title}}
          type: doc
          tags: []
          project: ""
          open_dir: ""
          code_workspace: ""
          terminal_dir: ""
          created: "{{date}}T{{time}}"
          pinned: false
          archived: false
          ---
          # {{title}}

          ## 想法

          ## 待办

          ```
        - `DEFAULT_TEMPLATE_COMMAND_BOOK`：
          ````markdown
          ---
          id: {{id}}
          title: {{title}}
          type: command-book
          tags:
            - commands
          project: ""
          open_dir: ""
          created: "{{date}}T{{time}}"
          ---
          # {{title}}

          ## 常用

          ### 示例命令

          命令用途说明。

          ```bash
          echo "hello quicknote"
          ```
          <!-- keywords: 示例 demo -->
          ````
      - `pub struct TemplateVars<'a> { pub id: &'a str, pub title: &'a str, pub date: &'a str, pub time: &'a str }`
      - `pub fn builtin_template(name: &str) -> Option<&'static str>`：支持 `"note"` / `"doc"` / `"command-book"`（容忍带 `.md` 后缀）。
      - `pub fn infer_default_template(dir_rel_path: &str) -> &'static str`：路径规范化后，若等于 `"commands"` 或以 `"commands/"` 开头返回 `"command-book"`；若等于 `"projects"` 或以 `"projects/"` 开头返回 `"doc"`；其余返回 `"note"`。
      - `pub fn render_template(raw: &str, vars: &TemplateVars<'_>) -> String`：
        - 替换 `{{id}}`、`{{title}}`、`{{date}}`、`{{time}}`（本地 `YYYY-MM-DD` 与 `HH:mm:ss`）；
        - **YAML 安全细节**：使用 `split_frontmatter(raw)` 区分 frontmatter 区与 body 区：在 frontmatter 内替换裸 `{{title}}`（未紧邻双引号时）若 `vars.title` 含 YAML 特殊字符（`:`、`#`、`"`、`'`、`[`、`]`、`{`、`}`、首尾空白等）则用 `render_yaml_string(vars.title)` 安全包裹，确保任何中文/带冒号标题生成的笔记均可被 `serde_yaml` 100% 合法解析；在 body 中直接替换原始 `vars.title`（保留 `# 我的: 标题` 原始外观）。
    - 更新 `src-tauri/src/infra/fs.rs::ensure_library_skeleton`：复用 `DEFAULT_TEMPLATE_NOTE`、`DEFAULT_TEMPLATE_DOC`、`DEFAULT_TEMPLATE_COMMAND_BOOK`，且继续严格保持“已有文件绝不覆盖”。
    - 修复 `src-tauri/src/application/index.rs` 与 `application/notes.rs` 中的 `_templates/` 索引与查询：
      - 在 `index.rs::build_note_index_record` 中，当 `rel_path.starts_with("_templates/")` 时：强制 `doc_type = NoteType::Template`、`key = format!("path:{rel_path}")`、`id = None`、清除因模板占位符 `id: {{id}}` / `title: {{title}}` 产生的误报 `meta_error`（模板文件名 stem 作为展示 `title`），防止模板文件污染真实笔记的重复 `id` 校验；
      - 在 `notes.rs::notes_list` 中：支持 `tab == "templates"`（匹配 `row.rel_path.starts_with("_templates/")`），并在 `"projects"`、`"tags"`、`"recent"`、`"workspaces"` 与 `_` 分支排除 `row.rel_path.starts_with("_templates/")`。
    - 升级 `src-tauri/src/application/notes.rs::note_create`：
      - 读取 `template_name`：若 `Some(name)` 非空，校验不含 `/`、`\`、`..`（非法返回 `ValidationError`），优先读取 `<library_root>/_templates/{clean_name}.md`，不存在时若命中内置名（`note`/`doc`/`command-book`）回退 `builtin_template`，否则返回 `ErrorCode::NotFound`；若 `template_name` 为 `None`，默认取 `"note"`（优先读 `_templates/note.md` 再回退内置）；
      - 使用本地时间 `Local::now()` 生成 `date`（`%Y-%m-%d`）、`time`（`%H:%M:%S`）及新 ULID，调用 `render_template` 渲染整篇文档；
      - 若 `args.body` 为 `Some(extra)` 且 `!extra.trim().is_empty()`：将其干净拼接到渲染后模板正文末尾（保持空行分隔，不覆盖模板 frontmatter 与标题）；
      - 保持原有原子写盘、`index_single_note` 与 `record_self_write`。
    - 修复 `src-tauri/src/application/notes.rs::note_update_meta`：
      - 当 `parsed.block.is_none()`（例如外部 `.md` 文件尚无 frontmatter，并非 YAML 损坏）时，不误写入 `backups/`，直接按 `merge_meta_with_patch` + `build_minimal_yaml` 生成最小 frontmatter 块并组合原正文写回；
      - 补全 `build_minimal_yaml`：完整输出 `id`、`title`、`type`、`tags`（块列表 `- tag` 或 `[]`）、`project`、`open_dir`、`code_workspace`、`terminal_dir`、`created`（双引号 ISO 字符串）、`pinned`、`archived`（`C6.4`）。
  - **3. 新增 `note_save_image` IPC 命令（`C4.2`、`v4` §5.6.3）**：
    - 在 `src-tauri/src/domain/note.rs` 新增 IPC 结构体（`rename_all = "camelCase"`, `specta::Type`）：
      - `SaveImageArgs { pub source_path: Option<String>, pub bytes_base64: Option<String>, pub ext: String, pub context_rel_path: Option<String> }`
      - `SavedImageResult { pub rel_path: String }`
    - 在 `src-tauri/src/domain/markdown.rs` 新增纯函数（不引入新 crate，零 `unwrap`）：
      - `pub fn normalize_image_ext(raw_ext: &str) -> Result<String, AppError>`：去除前导 `.` 转小写，仅允许白名单 `["png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp", "ico"]`，否则 `ValidationError`；
      - `pub fn decode_base64_image(raw: &str, max_bytes: usize) -> Result<Vec<u8>, AppError>`：自动剥离可选 `data:image/...;base64,` 前缀与空白符，严格执行 RFC 4648 base64 解码，超 `max_bytes`（默认 `20 * 1024 * 1024`）或非法字符返回 `ValidationError`。
    - 在 `src-tauri/src/application/notes.rs` 实现 `pub fn note_save_image(pm: &PersistenceManager, args: SaveImageArgs) -> Result<SavedImageResult, AppError>`：
      - 要求库根已配置（否则 `LibraryNotConfigured`）；
      - 从 `bytes_base64`（优先）或 `source_path`（读取磁盘文件并检查大小 ≤ 20MB）获取字节；两者皆空返回 `ValidationError`；
      - 按 `Local::now()` 生成路径 `assets/YYYY-MM/<YYYYMMDD-HHmmss>-<short_ulid>.<ext>`（`<short_ulid>` 取新 ULID 后 8 位小写），通过 `atomic_write_bytes` 原子落盘，返回 `SavedImageResult { rel_path }`（POSIX 相对路径）。
    - 在 `src-tauri/src/commands/notes.rs` 暴露 `#[tauri::command] pub fn note_save_image(...)`。
  - **4. 新增 `inbox_extract_to_note` IPC 命令（`C3.4`、`C4.2`、`v4` §4.11）**：
    - 在 `src-tauri/src/domain/capture.rs` 新增 IPC 结构体（`rename_all = "camelCase"`, `specta::Type`）：
      - `InboxExtractArgs { pub from_line: u32, pub to_line: u32, pub new_title: String, pub target_dir_rel_path: String, pub template_name: String }`
    - 在 `src-tauri/src/domain/capture.rs` 新增纯函数：
      - `pub fn extract_body_lines(body: &str, from_line: u32, to_line: u32) -> Result<(String, String), AppError>`：
        - 1-based 闭区间 `[from_line, to_line]`；校验 `from_line >= 1 && from_line <= to_line && to_line <= total_lines` 且提取文本 `trim()` 非空（否则 `ValidationError`）；
        - 返回 `(extracted_text, remaining_body)`：移除切口处连续多余空行（最多保留 1 个空行）并规范结尾 `\n`。
    - 在 `src-tauri/src/application/capture.rs` 实现 `pub fn inbox_extract_to_note(pm: &PersistenceManager, args: InboxExtractArgs) -> Result<NoteDoc, AppError>`：
      - 读取 `<library_root>/inbox.md`，`parse_note` 分离 `block + body`；
      - 调用 `extract_body_lines(&parsed.body, args.from_line, args.to_line)`；
      - **防丢数据顺序**：先调用 `application::notes::note_create` 创建目标新笔记（传入 `title = args.new_title`、`dir_rel_path = args.target_dir_rel_path`、`template_name = Some(args.template_name)`、`body = Some(extracted)`）——若目标重名或模板无效直接报错退出，`inbox.md` 毫发无伤；
      - 新笔记成功后，保留 `inbox.md` 原始 `parsed.block` 原样写回 `remaining_body`（`compose_note(parsed.block.as_deref(), &remaining_body)`），原子落盘、调用 `index_single_note` 更新索引、调用 `pm.record_self_write(&inbox_abs, &compute_body_hash(&remaining_body))` 防回环；
      - 返回新创建的 `NoteDoc`。
    - 在 `src-tauri/src/commands/capture.rs` 暴露 `#[tauri::command] pub fn inbox_extract_to_note(...)`。
  - **5. 绑定导出、IPC 封装与原生菜单同源转发（`C1.3`、`C1.4`、`C3.2`）**：
    - 更新 `src-tauri/src/bindings.rs` 注册全部新增 IPC 结构体（`SettingsPatch` 及其子 Patch、`SaveImageArgs`、`SavedImageResult`、`InboxExtractArgs`）。
    - 在 `src-tauri/src/lib.rs`：
      - 注册 `commands::settings::settings_update`、`commands::notes::note_save_image`、`commands::capture::inbox_extract_to_note`；
      - 完善 `build_app_menu` 中“文件”/“格式”/“视图”子菜单项，使其 ID 与前端 `C7.2` 统一动作 ID 精确对齐：`window.newNote`（`CmdOrCtrl+N`）、`editor.bold`、`editor.italic`、`editor.codeBlock`、`editor.table`、`editor.toggleTodo`、`editor.link`、`editor.image`、`editor.bulletList`、`editor.blockquote`、`editor.heading1`..`6`、`panel.toggle.outline`、`panel.collapseGroup`；
      - 在 `on_menu_event` 中，对上述前端动作 ID 调用 `app_handle.emit("menu:action", menu_id)` 转发至前端。
    - 运行 `pnpm gen:bindings` 更新 `src/bindings/types.ts`（零手改）。
    - 在 `src/lib/ipc/commands.ts` 新增 `settingsUpdate`、`noteSaveImage`、`inboxExtractToNote`；在 `src/lib/ipc/events.ts` 新增 `onMenuAction(handler: (actionId: string) => void): Promise<UnlistenFn>`。
- 不做：
  - 不新增 `Cargo.toml` 第三方依赖（base64 解码用纯 Rust 实现）。
  - 不实现前端设置表单、文档头 UI、右键菜单或 CM6 扩展（分别归 `M2-02`–`M2-05b`）。
  - 不修改 `C3.4` 既有 `NoteDoc` / `Settings` 字段结构。

## 2. 允许修改文件白名单
- 共享串行文件（`C1.3` 显式声明）：`src-tauri/src/bindings.rs`、`src/bindings/types.ts`（仅由 `pnpm gen:bindings` 生成）、`src/lib/ipc/commands.ts`、`src/lib/ipc/events.ts`
- Rust 生产源码：
  - `src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/settings.rs`、`src-tauri/src/domain/template.rs`（新增）、`src-tauri/src/domain/note.rs`、`src-tauri/src/domain/markdown.rs`、`src-tauri/src/domain/capture.rs`
  - `src-tauri/src/infra/fs.rs`
  - `src-tauri/src/application/persistence.rs`、`src-tauri/src/application/index.rs`、`src-tauri/src/application/notes.rs`、`src-tauri/src/application/capture.rs`
  - `src-tauri/src/commands/settings.rs`、`src-tauri/src/commands/notes.rs`、`src-tauri/src/commands/capture.rs`
  - `src-tauri/src/lib.rs`
- Rust 测试源码：`src-tauri/tests/m2_backend_test.rs`（新增）

## 3. 具体实现要求
- **零 `unwrap()` / `expect()` 与隐私红线（`C2.1`、`C2.3`）**：`src-tauri/src/**` 严禁出现 `.unwrap()` / `.expect()` / `panic!()`；日志严禁打印笔记正文、base64 内容或模板正文。
- **`domain` 纯净隔离（`C1.2`）**：`domain/template.rs`、`domain/settings.rs`、`domain/markdown.rs`、`domain/capture.rs` 严禁引入 `std::fs`、`tauri`、`rusqlite`。
- **保持 M0/M1 既有单测 100% 兼容**：确认 `persistence_test.rs`、`sqlite_migration_test.rs`、`notes_and_fts_test.rs`、`capture_and_undo_test.rs`、`watch_and_recovery_test.rs` 全部零回归通过。

## 4. 自动化验收标准（必须全绿）
- 新增测试文件：`src-tauri/tests/m2_backend_test.rs`，覆盖以下断言：
  1. `settings_update`：部分字段 Patch 合并落盘后重新 `PersistenceManager::open` 可完整还原；越界参数（如 `font_size: 4`、`bloat_threshold_lines: 2`、`auto_save_max_wait_ms < debounce`）返回 `ValidationError`；当磁盘 `settings.yaml` 存在语法错误时，`settings_update` 返回 `SettingsParseError` 且磁盘原坏文件内容一字未改（`D-0008`）。
  2. 默认模板与 B.3 对齐 + 模板渲染：校验 `ensure_library_skeleton` 生成的 `_templates/note.md`、`_templates/doc.md`、`_templates/command-book.md` 精确包含 `v4` 附录 B.3 要求字段（`pinned: false`、`archived: false`、`project: ""`、`open_dir: ""`、`## 常用`、`<!-- keywords: 示例 demo -->`）；验证 `render_template` 正确替换 `{{id}}/{{title}}/{{date}}/{{time}}` 且含冒号标题（如 `项目: v2`）生成的 frontmatter 可正常解析；验证 `note_create` 使用自定义 `_templates/meeting.md` 及附加 `body` 成功建文；验证 `notes_list(tab="templates")` 返回模板列表且 `tab="recent"` 不混入模板。
  3. `note_update_meta` 补全校验：对无 frontmatter 外部文件调用 `note_update_meta` 设置 `project`/`open_dir`/`code_workspace`/`terminal_dir`/`pinned` 后不生成误报备份且全部字段可读回；对已有未知扩展键的库内笔记更新元数据后未知键与注释 100% 保留。
  4. `note_save_image`：验证 `bytes_base64`（含 `data:image/png;base64,` 前缀）与 `source_path` 两种方式均正确落盘 `assets/YYYY-MM/*.png` 且字节一致；非法扩展名（如 `exe`/`sh`）或超限负载返回 `ValidationError`。
  5. `inbox_extract_to_note`：验证从 `inbox.md` 提取指定行区间 `[from_line, to_line]` 到指定目录与模板新建笔记后，新笔记包含提取正文且保留模板 frontmatter，`inbox.md` 对应行已被剪切且保留原 frontmatter，同时登记防回环缓存；若目标重名导致建文失败，`inbox.md` 内容保持原样未丢失。
- 必过命令：`pnpm check:all`（含 `pnpm gen:bindings` 零差异、`check:arch`、`cargo test`、`vitest`）。

## 5. 人工点验清单（如涉及 macOS 原生行为）
- [ ] 启动开发版 `pnpm tauri dev`，打开顶部系统菜单栏「格式」子菜单，确认已列出加粗、斜体、代码块、表格、切换待办、插入链接、插入图片、无序列表、引用块、标题 1–6 且带对应快捷键提示。
