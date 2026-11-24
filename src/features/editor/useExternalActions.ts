import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  ExternalActionId,
  ExternalActionState,
  NoteDoc,
  RunExternalArgs,
} from '@/bindings/types';
import type { ContextMenuEntry } from '@/components/QnContextMenu.vue';
import { useNotesStore } from '@/features/notes';
import { extractEditorErrorMessage, resolveExternalActions, runExternalAction } from './api';
import { useEditorTabsStore } from './store';

export interface ExternalActionSpec {
  id: ExternalActionId;
  title: string;
  shortcutLabel?: string;
  pathMode: 'file' | 'dir';
}

export const EXTERNAL_ACTION_SPECS: readonly ExternalActionSpec[] = [
  { id: 'external.reveal', title: '在访达中显示', shortcutLabel: '⇧⌘O', pathMode: 'file' },
  { id: 'external.terminal', title: '在外部终端中打开', shortcutLabel: '⌥⌘T', pathMode: 'dir' },
  { id: 'external.vscode', title: '在 VS Code 中打开', pathMode: 'dir' },
  { id: 'external.sublime', title: '在 Sublime Text 中打开', pathMode: 'dir' },
  { id: 'external.workspace', title: '在外部编辑器打开工作区', pathMode: 'file' },
  { id: 'external.default', title: '使用系统默认应用打开', pathMode: 'file' },
  { id: 'external.copyPath', title: '复制文件路径', pathMode: 'file' },
  { id: 'external.pin', title: '固定 / 取消固定常用文件', pathMode: 'file' },
];

const EXTERNAL_SPEC_MAP = new Map<ExternalActionId, ExternalActionSpec>(
  EXTERNAL_ACTION_SPECS.map((spec) => [spec.id, spec]),
);

export interface ExternalActionFeedback {
  tone: 'success' | 'warning' | 'danger';
  message: string;
}

export function isExternalActionId(id: string): id is ExternalActionId {
  return EXTERNAL_SPEC_MAP.has(id as ExternalActionId);
}

export function buildExternalRunArgs(
  id: ExternalActionId,
  state: ExternalActionState,
): RunExternalArgs {
  const spec = EXTERNAL_SPEC_MAP.get(id);
  if (spec?.pathMode === 'dir') {
    return {
      id,
      absPath: null,
      dirPath: state.resolvedPath,
    };
  }
  return {
    id,
    absPath: state.resolvedPath,
    dirPath: null,
  };
}

export function buildExternalContextMenuItems(
  actionStates: readonly ExternalActionState[],
): ContextMenuEntry[] {
  const stateMap = new Map<ExternalActionId, ExternalActionState>(
    actionStates.map((item) => [item.id, item]),
  );

  return EXTERNAL_ACTION_SPECS.map((spec) => {
    const state = stateMap.get(spec.id);
    const enabled = Boolean(state?.enabled && state?.resolvedPath);
    return {
      type: 'item',
      id: spec.id,
      title: spec.title,
      shortcutLabel: spec.shortcutLabel,
      disabled: !enabled,
      disabledReason: enabled ? null : (state?.disabledReason ?? '当前文档不可用'),
    };
  });
}

export const useExternalActions = defineStore('externalActions', () => {
  const states = ref<ExternalActionState[]>([]);
  const resolvedAbsPath = ref<string | null>(null);
  const loading = ref<boolean>(false);
  const feedback = ref<ExternalActionFeedback | null>(null);

  let resolveSeq = 0;

  const stateMap = computed(() => {
    const map = new Map<ExternalActionId, ExternalActionState>();
    for (const item of states.value) {
      map.set(item.id, item);
    }
    return map;
  });

  const menuItems = computed<ContextMenuEntry[]>(() => buildExternalContextMenuItems(states.value));

  function isActionEnabled(id: ExternalActionId): boolean {
    const state = stateMap.value.get(id);
    return Boolean(state?.enabled && state?.resolvedPath);
  }

  function getDisabledReason(id: ExternalActionId): string | null {
    const state = stateMap.value.get(id);
    if (!state || state.enabled) {
      return null;
    }
    return state.disabledReason;
  }

  async function refreshForDoc(
    doc: Pick<NoteDoc, 'absPath' | 'meta'> | null,
  ): Promise<ExternalActionState[]> {
    const reqSeq = ++resolveSeq;
    if (!doc?.absPath) {
      states.value = [];
      resolvedAbsPath.value = null;
      loading.value = false;
      return [];
    }

    loading.value = true;
    try {
      const nextStates = await resolveExternalActions({
        absPath: doc.absPath,
        workspaceKey: doc.meta.project ?? null,
        commandId: null,
      });
      if (reqSeq !== resolveSeq) {
        return states.value;
      }
      states.value = nextStates;
      resolvedAbsPath.value = doc.absPath;
      return nextStates;
    } catch (error) {
      if (reqSeq !== resolveSeq) {
        return states.value;
      }
      states.value = [];
      feedback.value = {
        tone: 'danger',
        message: extractEditorErrorMessage(error, '解析外部动作状态失败'),
      };
      return [];
    } finally {
      if (reqSeq === resolveSeq) {
        loading.value = false;
      }
    }
  }

  async function resolveForTarget(target: {
    absPath: string | null;
    workspaceKey: string | null;
  }): Promise<ExternalActionState[]> {
    if (!target.absPath) {
      return [];
    }
    return resolveExternalActions({
      absPath: target.absPath,
      workspaceKey: target.workspaceKey,
      commandId: null,
    });
  }

  async function executeAction(
    id: ExternalActionId,
    options?: {
      overrideStates?: readonly ExternalActionState[];
      pinned?: boolean;
    },
  ): Promise<boolean> {
    const targetList = options?.overrideStates ?? states.value;
    const state = targetList.find((item) => item.id === id);

    if (!state || !state.enabled || !state.resolvedPath) {
      if (state?.disabledReason) {
        feedback.value = {
          tone: 'warning',
          message: state.disabledReason,
        };
      }
      return false;
    }

    try {
      await runExternalAction(buildExternalRunArgs(id, state));
      if (id === 'external.copyPath') {
        feedback.value = { tone: 'success', message: '已复制文件路径' };
      } else if (id === 'external.pin') {
        let wasPinned = options?.pinned;
        try {
          const tabsStore = useEditorTabsStore();
          if (wasPinned === undefined) {
            const matchedTab = tabsStore.tabs.find((tab) => tab.absPath === state.resolvedPath);
            wasPinned = matchedTab?.doc?.meta.pinned ?? false;
          }
          const nextPinned = !wasPinned;
          for (const tab of tabsStore.tabs) {
            if (tab.absPath === state.resolvedPath && tab.doc) {
              tab.doc = {
                ...tab.doc,
                meta: {
                  ...tab.doc.meta,
                  pinned: nextPinned,
                },
              };
            }
          }
          feedback.value = {
            tone: 'success',
            message: nextPinned ? '已固定到常用文件' : '已取消固定常用文件',
          };
        } catch {
          const nextPinned = !wasPinned;
          feedback.value = {
            tone: 'success',
            message: nextPinned ? '已固定到常用文件' : '已取消固定常用文件',
          };
        }
        try {
          const notesStore = useNotesStore();
          void notesStore.refreshPinnedExternalFiles();
        } catch {
          // 独立单元测试环境容错
        }
      } else {
        const spec = EXTERNAL_SPEC_MAP.get(id);
        feedback.value = {
          tone: 'success',
          message: spec ? `已执行：${spec.title}` : '外部动作已执行',
        };
      }
      return true;
    } catch (error) {
      feedback.value = {
        tone: 'danger',
        message: extractEditorErrorMessage(error, '执行外部动作失败'),
      };
      return false;
    }
  }

  function clearFeedback(): void {
    feedback.value = null;
  }

  return {
    states,
    resolvedAbsPath,
    loading,
    feedback,
    menuItems,
    isActionEnabled,
    getDisabledReason,
    refreshForDoc,
    resolveForTarget,
    executeAction,
    clearFeedback,
  };
});
