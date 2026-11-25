# M5-03c：应用图标与打包配置（R37、R38、R39、R40）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1；C1.3、C9.1
- 前置依赖：M5-01b（门禁已转绿、版本号已对齐）
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：
  - R37：按应用主题生成简洁的应用图标，接入项目。
  - R40：`pnpm tauri build` 直接产出完整安装包，不再需要临时改 `bundle.active`。
  - R38：新增一条只编译出 `.app`、不出 DMG 的命令。
  - R39：盘点打包产物，能安全精简的精简，不能的记录原因。
- 不做：不改 CI、不改签名 / 公证配置、不改 `cargo test` 相关配置。

## 2. 允许修改文件白名单

- `src-tauri/icons/`（源图 + 由 Tauri CLI 生成的一整套图标，视为一个资产目录）
- `src-tauri/tauri.conf.json`
- `src-tauri/tauri.app.conf.json`（新建）
- `package.json`
- `README.md`（只改第 5 节「本地开发、验收与 macOS 打包」）
- `scripts/`（仅当 R39 需要一个盘点脚本时新建）

## 3. 具体实现要求

### 3.1 R37 图标

1. 新建源图 `src-tauri/icons/app-icon.svg`，1024×1024，设计规格（照此实现，保证简洁）：
   - 背景：圆角正方形，`rx=228`，填充 `#2563eb`（浅色主题 `--qn-accent`）。
   - 主体：居中的白色圆角矩形「便签」，宽 520、高 640，`rx=72`，中心与画布中心重合。
   - 便签上三条蓝色横线（`#2563eb`），宽 280、高 40、`rx=20`，垂直间距 110。
   - 便签右下角叠一个白色描边的蓝色实心圆点（半径 96，填充 `#ffffff`，圆心在便签右下角内侧），表达「快速捕捉」。
   - 画布四角保持透明。
2. 运行 `pnpm tauri icon src-tauri/icons/app-icon.svg`。若当前 CLI 不接受 SVG 输入，改用 `qlmanage -t -s 1024 -o /tmp src-tauri/icons/app-icon.svg` 得到 PNG 后再执行一次。
3. 确认 `src-tauri/icons/` 下生成了 `32x32.png`、`128x128.png`、`128x128@2x.png`、`icon.icns`。
4. 旧的占位 `icon.png`（104 字节）删除。

### 3.2 R40 打包开关

`tauri.conf.json` 的 `bundle` 段改为：

```json
"bundle": {
  "active": true,
  "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns"]
}
```

### 3.3 R38 只出 `.app`

1. 新建 `src-tauri/tauri.app.conf.json`：

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "bundle": { "targets": ["app"] }
}
```

2. `package.json` 的 `scripts` 增加 `"build:app": "tauri build --config src-tauri/tauri.app.conf.json"`。`"tauri": "tauri"` 保持不变。

### 3.4 实跑

依次执行并确认成功：

- `pnpm tauri build`：产出 `.app` 与 `.dmg`。
- `pnpm build:app`：只产出 `.app`。

把两条命令的产物路径写进提交说明。

### 3.5 R39 产物盘点

对 `pnpm build:app` 的产物做盘点，逐项判断并记录：

| 可疑项 | 怎么查 | 处理原则 |
| --- | --- | --- |
| 多余语言包（`.lproj`） | `ls <App>/Contents/Resources/*.lproj` | 若 Tauri 无官方配置可在签名前裁剪，**不要**在构建后删（会破坏应用签名）。记录数量与体积，结论写「保留」并说明原因。 |
| 其他平台编译文件 | 检查仓库里 `src-tauri/icons/` 是否被 `tauri icon` 生成了 Windows / Android / iOS 专用图标 | macOS 专用的四个文件保留；其余平台图标与 `icon.ico` 不在 `bundle.icon` 引用，可删除不用的，只保留 `icon.ico` 若 CLI 要求存在。 |
| 用不到的依赖 | `package.json` 的 `dependencies` 与 `Cargo.toml` 的 `[dependencies]` 逐个 `grep` 确认有引用 | 只删除**确认零引用**的项；拿不准的保留并记录。 |
| 其他体积异常 | `du -sh <App>/Contents/*` | 只记录，不处理。 |

盘点结果以表格形式写进提交说明，并在 `README.md` 第 5 节补一段「打包产物说明」。

### 3.6 README

`README.md` 第 5 节：

- 删除「打包前将 `src-tauri/tauri.conf.json` 中 `bundle.active` 设为 `true`」这句。
- 改为说明两条命令：`pnpm tauri build`（出 `.app` + `.dmg`）与 `pnpm build:app`（只出 `.app`）。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿（重点确认第 9 步 `cargo test` 在 `bundle.active: true` 下照常通过）。
- `pnpm tauri build` 与 `pnpm build:app` 均成功。
- `src-tauri/tauri.app.conf.json` 与 `tauri.conf.json` 能通过 Tauri 的 schema 校验（构建成功即通过）。

## 5. 人工点验清单

- [ ] 打开产出的 `.app`：Dock 图标为新生成的图标（不是默认的 Tauri 图标）。
- [ ] 访达「显示简介」里显示版本 `2.0.0`。
- [ ] DMG 可正常挂载并拖入「应用程序」。
- [ ] 45% 侧栏辅助模式全屏 Space 下，窗口切换正常（确认 `bundle` 配置未影响运行时行为）。

## 上下文提示

- 图标以规格为准，不要自行改设计。生成图标后目视检查一次：蓝色圆角方块、白色便签、三条蓝线。
- 忽略：代码签名、公证、自动更新。
