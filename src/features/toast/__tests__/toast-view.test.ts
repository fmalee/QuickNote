import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ToastAppendUndoPayload } from '@/features/toast';

const { mockListenEvent } = vi.hoisted(() => ({
  mockListenEvent: vi.fn(),
}));

vi.mock('@/lib/ipc/events', () => ({
  listenEvent: mockListenEvent,
  onSettingsStatus: vi.fn(),
  onWindowState: vi.fn(),
}));

import { ToastView, useToastStore } from '@/features/toast';

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

    let capturedHandler: ((payload: ToastAppendUndoPayload) => void) | undefined;
    mockListenEvent.mockImplementationOnce(
      async (_event: string, handler: (payload: ToastAppendUndoPayload) => void) => {
        capturedHandler = handler;
        return () => {};
      },
    );

    const store = useToastStore();
    await store.initialize();

    expect(mockListenEvent).toHaveBeenCalledWith('toast:append-undo', expect.any(Function));

    capturedHandler?.({
      relPath: 'inbox.md',
      title: '收件箱',
      addedLines: 1,
      sectionHeading: '2026-11-23',
      undoAvailable: true,
    });

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

    let capturedHandler: ((payload: ToastAppendUndoPayload) => void) | undefined;
    mockListenEvent.mockImplementationOnce(
      async (_event: string, handler: (payload: ToastAppendUndoPayload) => void) => {
        capturedHandler = handler;
        return () => {};
      },
    );

    const store = useToastStore();
    await store.initialize();

    capturedHandler?.({
      relPath: 'inbox.md',
      title: '收件箱',
      addedLines: 1,
      sectionHeading: '2026-11-23',
      undoAvailable: true,
    });

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
});
