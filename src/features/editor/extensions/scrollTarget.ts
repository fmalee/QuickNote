import type { Text } from '@codemirror/state';
import type { EditorScrollTarget } from '../store';

export interface ResolvedScrollPosition {
  anchor: number;
  line: number;
}

/**
 * 解析跨窗口跳转/大纲/诊断定位目标在 CodeMirror 文档中的字符偏移与 1-based 行号。
 */
export function resolveScrollTargetAnchor(
  doc: Text,
  target: Pick<EditorScrollTarget, 'from' | 'line'>,
): ResolvedScrollPosition {
  if (target.from <= 0 && target.line > 0) {
    const maxLine = Math.max(1, doc.lines);
    const clampedLine = Math.max(1, Math.min(target.line, maxLine));
    const lineObj = doc.line(clampedLine);
    return {
      anchor: lineObj.from,
      line: lineObj.number,
    };
  }
  const maxOffset = doc.length;
  const anchor = Math.max(0, Math.min(target.from, maxOffset));
  return {
    anchor,
    line: target.line > 0 ? target.line : doc.lineAt(anchor).number,
  };
}
