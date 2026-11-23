import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppendResult } from '@/bindings/types';

const { mockOnToastAppendUndo, mockNoteUndoLastAppend } = vi.hoisted(() => ({
  mockOnToastAppendUndo: vi.fn(),
  mockNoteUndoLastAppend: vi.fn(),
}));

vi.mock('@/lib/ipc/events', () => ({
  onToastAppendUndo: mockOnToastAppendUndo,
  onSettingsStatus: vi.fn(),
  onWindowState: vi.fn(),
  onIndexStatus: vi.fn(),
}));

vi.mock('@/lib/ipc/commands', () => ({
  noteUndoLastAppend: mockNoteUndoLastAppend,
}));

import { ToastView, useToastStore } from '@/features/toast';

function sampleAppendResult(overrides: Partial<AppendResult> = {}): AppendResult {
  return {
    relPath: 'inbox.md',
    title: '收件箱',
    addedLines: 1,
    sectionHeading: '## 2026-11-24',
    bodyHash: 'abc123',
    mtimeMs: 1_732_400_000_000,
    undoAvailable: true,
    ...overrides,
  };
}

describe('toast-view (toast:append-undo)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('displays target document name and [撤销] button upon receiving toast:append-undo', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    let capturedHandler: ((payload: AppendResult) => void) | undefined;
    mockOnToastAppendUndo.mockImplementationOnce(
      async (handler: (payload: AppendResult) => void) => {
        capturedHandler = handler;
        return () => {};
      },
    );

    const store = useToastStore();
    await store.initialize();

    expect(mockOnToastAppendUndo).toHaveBeenCalledWith(expect.any(Function));

    capturedHandler?.(sampleAppendResult());

    expect(store.visible).toBe(true);
    expect(store.payload?.title).toBe('收件箱');

    const app = createSSRApp(ToastView);
    app.use(pinia);
    const html = await renderToString(app);
    expect(html).toContain('已追加至「收件箱」');
    expect(html).toContain('撤销');

    store.dispose();
  });

  it('counts down 5 seconds with fake timers and auto-hides', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    let capturedHandler: ((payload: AppendResult) => void) | undefined;
    mockOnToastAppendUndo.mockImplementationOnce(
      async (handler: (payload: AppendResult) => void) => {
        capturedHandler = handler;
        return () => {};
      },
    );

    const store = useToastStore();
    await store.initialize();

    capturedHandler?.(sampleAppendResult());

    expect(store.visible).toBe(true);
    expect(store.remainingSeconds).toBe(5);

    vi.advanceTimersByTime(1000);
    expect(store.remainingSeconds).toBe(4);
    expect(store.visible).toBe(true);

    vi.advanceTimersByTime(3000);
    expect(store.remainingSeconds).toBe(1);
    expect(store.visible).toBe(true);

    vi.advanceTimersByTime(1000);
    expect(store.visible).toBe(false);

    store.dispose();
  });

  it('invokes note_undo_last_append IPC when triggerUndo is called', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);

    mockOnToastAppendUndo.mockResolvedValueOnce(() => {});
    mockNoteUndoLastAppend.mockResolvedValueOnce({
      relPath: 'inbox.md',
      restoredBodyHash: 'restored-hash',
      mtimeMs: 1_732_400_005_000,
    });

    const store = useToastStore();
    await store.initialize();
    store.showPayload(sampleAppendResult());

    await store.triggerUndo();

    expect(store.undoTriggered).toBe(true);
    expect(store.visible).toBe(false);
    expect(mockNoteUndoLastAppend).toHaveBeenCalledTimes(1);

    store.dispose();
  });
});
