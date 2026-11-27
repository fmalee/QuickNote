import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import type { NoteDoc } from '@/bindings/types';
import { QnToastContainer } from '@/components';
import { DocHeaderBar, useEditorTabsStore, useExternalActions } from '@/features/editor';
import { useNotificationStore } from '@/lib/notifications';

const mockExternalResolve = vi.fn();
const mockExternalRun = vi.fn();
const mockExternalBookmarksList = vi.fn();

vi.mock('@/lib/ipc/commands', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/ipc/commands')>();
  return {
    ...actual,
    externalActionsResolve: (...args: unknown[]) => mockExternalResolve(...args),
    externalActionRun: (...args: unknown[]) => mockExternalRun(...args),
    externalBookmarksList: (...args: unknown[]) => mockExternalBookmarksList(...args),
  };
});

function makeDoc(overrides?: Partial<NoteDoc>): NoteDoc {
  return {
    key: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
    relPath: 'notes/demo.md',
    absPath: '/vault/notes/demo.md',
    isLibrary: true,
    isInbox: false,
    meta: {
      id: '01ARZ3NDEKTSV4RRFFQ69G5FAV',
      title: '演示笔记',
      docType: 'note',
      tags: ['work'],
      openDir: null,
      codeWorkspace: null,
      terminalDir: null,
      created: '2026-01-01T00:00:00Z',
      pinned: false,
      archived: false,
      metaError: null,
    },
    body: '# Hello',
    bodyHash: 'hash-1',
    mtimeMs: 1000,
    sizeBytes: 7,
    ...overrides,
  };
}

describe('unified notification store & cross-module routing', () => {
  let pinia: ReturnType<typeof createPinia>;

  beforeEach(() => {
    vi.useFakeTimers();
    pinia = createPinia();
    setActivePinia(pinia);
    mockExternalResolve.mockReset().mockResolvedValue([]);
    mockExternalRun.mockReset().mockResolvedValue(undefined);
    mockExternalBookmarksList.mockReset().mockResolvedValue([]);
  });

  afterEach(() => {
    useNotificationStore().dispose();
    vi.useRealTimers();
  });

  it('auto-dismisses status notices, supports hover pause/resume, and replaces older status notices', () => {
    const store = useNotificationStore();

    store.notifyStatus({ tone: 'success', message: '已复制文件路径', durationMs: 3000 });
    expect(store.statusNotice?.message).toBe('已复制文件路径');
    expect(store.statusNotice?.tone).toBe('success');

    vi.advanceTimersByTime(1500);
    store.pauseStatusTimer();
    vi.advanceTimersByTime(5000);
    expect(store.statusNotice?.message).toBe('已复制文件路径');

    store.resumeStatusTimer();
    vi.advanceTimersByTime(2100);
    expect(store.statusNotice).toBeNull();

    store.notifyStatus({ tone: 'info', message: '消息 A', durationMs: 3000 });
    store.notifyStatus({ tone: 'warning', message: '消息 B', durationMs: 3000 });
    expect(store.statusNotice?.message).toBe('消息 B');
    store.clearStatus();
    expect(store.statusNotice).toBeNull();
  });

  it('deduplicates identical toasts, caps stack at 4, and supports manual dismissal', () => {
    const store = useNotificationStore();

    const firstId = store.notifyToast({
      tone: 'danger',
      title: '保存失败',
      message: '磁盘空间不足',
    });
    const dupId = store.notifyToast({
      tone: 'danger',
      title: '保存失败',
      message: '磁盘空间不足',
    });
    expect(dupId).toBe(firstId);
    expect(store.toasts).toHaveLength(1);

    for (let i = 1; i <= 5; i += 1) {
      store.notifyToast({ tone: 'warning', title: `提示 ${i}`, message: `内容 ${i}` });
    }
    expect(store.toasts).toHaveLength(4);
    expect(store.toasts[0]?.title).toBe('提示 2');

    const targetId = store.toasts[0]!.id;
    store.dismissToast(targetId);
    expect(store.toasts.some((item) => item.id === targetId)).toBe(false);
  });

  it('routes external action success/disabled to status bar and execution errors to toast', async () => {
    const notificationStore = useNotificationStore();
    const externalActions = useExternalActions();

    const disabledOk = await externalActions.executeAction('external.vscode', {
      overrideStates: [
        {
          id: 'external.vscode',
          enabled: false,
          disabledReason: '未配置关联工作区',
          resolvedPath: null,
        },
      ],
    });
    expect(disabledOk).toBe(false);
    expect(notificationStore.statusNotice?.tone).toBe('warning');
    expect(notificationStore.statusNotice?.message).toBe('未配置关联工作区');
    expect(notificationStore.toasts).toHaveLength(0);

    const ok = await externalActions.executeAction('external.copyPath', {
      overrideStates: [
        {
          id: 'external.copyPath',
          enabled: true,
          disabledReason: null,
          resolvedPath: '/vault/notes/demo.md',
        },
      ],
    });
    expect(ok).toBe(true);
    expect(notificationStore.statusNotice?.tone).toBe('success');
    expect(notificationStore.statusNotice?.message).toBe('已复制文件路径');
    expect(notificationStore.toasts).toHaveLength(0);

    mockExternalRun.mockRejectedValueOnce({
      code: 'permissionDenied',
      message: '无权打开目标应用',
      detail: null,
    });
    const failedOk = await externalActions.executeAction('external.reveal', {
      overrideStates: [
        {
          id: 'external.reveal',
          enabled: true,
          disabledReason: null,
          resolvedPath: '/vault/notes/demo.md',
        },
      ],
    });
    expect(failedOk).toBe(false);
    expect(notificationStore.toasts).toHaveLength(1);
    expect(notificationStore.toasts[0]?.tone).toBe('danger');
    expect(notificationStore.toasts[0]?.title).toBe('外部动作失败');
    expect(notificationStore.toasts[0]?.message).toContain('无权打开目标应用');
  });

  it('keeps DocHeaderBar free of duplicate feedback banners while preserving doc.meta.metaError', async () => {
    const editorTabsStore = useEditorTabsStore();
    const externalActions = useExternalActions();
    const doc = makeDoc({
      meta: {
        ...makeDoc().meta,
        metaError: 'YAML 语法错误',
      },
    });
    editorTabsStore.tabs = [
      {
        key: doc.key,
        relPath: doc.relPath,
        absPath: doc.absPath,
        isPinned: false,
        isPreview: false,
        cursorPos: 0,
        scrollTop: 0,
        doc,
        cachedTitle: doc.meta.title,
        isDirty: false,
        isMissing: false,
        draftBody: doc.body,
        baseBodyHash: doc.bodyHash,
        conflictState: null,
        saveStatus: 'saved',
        cursorLine: 1,
        cursorCol: 1,
        selectionFromLine: 1,
        selectionToLine: 1,
        hasSelection: false,
        syncSeq: 0,
      },
    ];
    editorTabsStore.activeKey = doc.key;
    editorTabsStore.errorMessage = '保存失败：磁盘只读';
    externalActions.feedback = { tone: 'success', message: '已复制文件路径' };

    const app = createSSRApp(DocHeaderBar);
    app.use(pinia);
    const html = await renderToString(app);

    expect(html).not.toContain('doc-header-external-feedback');
    expect(html).not.toContain('doc-header-copy-feedback');
    expect(html).not.toContain('保存失败：磁盘只读');
    expect(html).not.toContain('已复制文件路径');
    expect(html).toContain('editor-doc-meta-error');
    expect(html).toContain('YAML 语法错误');
  });

  it('renders workbench toast container with title and message', async () => {
    const app = createSSRApp({
      render: () =>
        h(QnToastContainer, {
          items: [
            {
              id: 42,
              tone: 'danger',
              title: '文档操作失败',
              message: '权限不足',
            },
          ],
        }),
    });
    const html = await renderToString(app);

    expect(html).toContain('workbench-toast-container');
    expect(html).toContain('workbench-toast-item');
    expect(html).toContain('文档操作失败');
    expect(html).toContain('权限不足');
  });
});
