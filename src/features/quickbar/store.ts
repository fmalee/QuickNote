import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  HideReason,
  LibraryInfo,
  QuickBarMode,
  SettingsLoadStatus,
  SettingsSnapshot,
} from '@/bindings/types';
import type { UnlistenFn } from '@/lib/ipc/events';
import {
  fetchQuickBarLibraryInfo,
  fetchQuickBarSettings,
  hideQuickBarWindow,
  showMainWindowForOnboarding,
  subscribeQuickBarWindowState,
} from './api';

export const useQuickBarStore = defineStore('quickbar', () => {
  const libraryInfo = ref<LibraryInfo | null>(null);
  const settingsSnapshot = ref<SettingsSnapshot | null>(null);
  const mode = ref<QuickBarMode>('capture');
  const draft = ref<string>('');
  const loading = ref<boolean>(false);
  const focusTick = ref<number>(0);

  let unlistenWindowState: UnlistenFn | null = null;

  const isUnconfigured = computed<boolean>(() => !libraryInfo.value?.root);
  const canSubmit = computed<boolean>(() => !isUnconfigured.value && draft.value.trim().length > 0);
  const settingsStatus = computed<SettingsLoadStatus | null>(
    () => settingsSnapshot.value?.status ?? null,
  );

  async function refreshState(): Promise<void> {
    loading.value = true;
    try {
      const [info, snapshot] = await Promise.all([
        fetchQuickBarLibraryInfo(),
        fetchQuickBarSettings(),
      ]);
      libraryInfo.value = info;
      settingsSnapshot.value = snapshot;
    } finally {
      loading.value = false;
    }
  }

  async function initialize(): Promise<void> {
    await refreshState();
    if (!unlistenWindowState) {
      unlistenWindowState = await subscribeQuickBarWindowState((state) => {
        if (state.windowId === 'quickbar' && state.visible) {
          void refreshState().then(() => {
            focusTick.value += 1;
          });
        }
      });
    }
    focusTick.value += 1;
  }

  function dispose(): void {
    if (unlistenWindowState) {
      unlistenWindowState();
      unlistenWindowState = null;
    }
  }

  async function openMainOnboarding(): Promise<void> {
    await showMainWindowForOnboarding();
  }

  async function hide(reason: HideReason = 'esc'): Promise<void> {
    await hideQuickBarWindow(reason);
  }

  return {
    libraryInfo,
    settingsSnapshot,
    mode,
    draft,
    loading,
    focusTick,
    isUnconfigured,
    canSubmit,
    settingsStatus,
    refreshState,
    initialize,
    dispose,
    openMainOnboarding,
    hide,
  };
});
