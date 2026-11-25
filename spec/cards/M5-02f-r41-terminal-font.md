# M5-02f：修复内嵌终端字距过大（R41）

- 所属里程碑：M5
- 关联方案与契约：`v5` §1.2；C7.1、C9.1；D-0005
- 前置依赖：M5-01a
- 会话策略：独立新会话执行；完成后提交并结束会话

## 1. 目标范围

- 要做：终端里中英文混排的字距恢复均匀紧凑。
- 不做：不改终端配色、字号设置项、不改 xterm 的滚动条处理。

## 2. 允许修改文件白名单

- `src/features/terminal/TerminalPanel.vue`
- `src/features/terminal/theme.ts`

## 3. 具体实现要求

### 3.1 现状

`TerminalPanel.vue` 基线约 93–100 行：

```ts
term = new Terminal({
  cursorBlink: true,
  scrollback: 5000,
  fontSize: 12,
  fontFamily: 'var(--qn-font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)',
  ...
```

两个嫌疑点，都要处理：

1. `fontFamily` 传的是 CSS `var(...)` 字符串。xterm 在测量字符宽度时可能把它用在 canvas 的 `font` 上，而 canvas 不支持 `var()`，导致测量失败、回退到默认字体宽度。
2. 字号 12px 时等宽字体（SF Mono / Menlo）的字符宽约 7.22px，xterm 会向上取整到 8px，每个字符多出约 0.78px 空隙。

### 3.2 修法

1. 在 `theme.ts` 新增导出 `resolveTerminalFontFamily(): string`：用 `getComputedStyle(document.documentElement).getPropertyValue('--qn-font-mono')` 读取真实字体栈并 `trim()`；结果为空（非浏览器环境）时回退 `'SF Mono, Menlo, monospace'`。
2. `TerminalPanel.vue` 的 `fontFamily` 改为 `resolveTerminalFontFamily()`。
3. 字号：默认改为 `13`（SF Mono 13px 字符宽 7.83px，取整后间隙约 0.17px）。在代码旁加一行注释说明原因。
4. 不设置 `letterSpacing`（保持 0）。

## 4. 自动化验收标准（必须全绿）

- `pnpm check:all` 全绿。

## 5. 人工点验清单

- [ ] 终端里输入 `ls -la /usr/local/etc` 与中文 `echo 中文测试`：字符间距均匀，无明显空隙，无字符重叠。
- [ ] 浅色与深色主题下各验一次，字体一致。
- [ ] 拖动改变终端面板尺寸后字符仍对齐网格（无竖向错位）。

## 上下文提示

- 只读 `TerminalPanel.vue` 约 85–110 行与 `theme.ts` 全文（147 行）。
- 忽略：终端会话生命周期、粘贴行为。
