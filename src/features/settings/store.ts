import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  EditorTheme,
  HotkeyStatus,
  SettingsLoadStatus,
  SettingsSnapshot,
} from '@/bindings/types';
import type { UnlistenFn } from '@/lib/ipc/events';
import { applyThemeToDocument } from '@/lib/utils';
import {
  extractSettingsErrorMessage,
  fetchSettingsSnapshot,
  fetchUiState,
  subscribeSettingsStatus,
  updateGlobalHotkey,
} from './api';

export type SettingsSection = 'general' | 'appearance' | 'baseline';

export const BACKUP_HOTKEY_COMBO = 'ctrl+alt+cmd+space';

export const useSettingsStore = defineStore('settings', () => {
  const snapshot = ref<SettingsSnapshot | null>(null);
  const hotkeyStatus = ref<HotkeyStatus | null>(null);
  const themePreference = ref<EditorTheme>('system');
  const activeSection = ref<SettingsSection>('general');
  const loading = ref<boolean>(false);
  const updatingHotkey = ref<boolean>(false);
  const hotkeyError = ref<string | null>(null);

  let unlistenStatus: UnlistenFn | null = null;

  const loadStatus = computed<SettingsLoadStatus | null>(() => snapshot.value?.status ?? null);
  const isSettingsInvalid = computed<boolean>(
    () => loadStatus.value !== null && !loadStatus.value.valid,
  );
  const isHotkeyConflict = computed<boolean>(() => hotkeyStatus.value?.selfCheck === 'conflict');

  function setTheme(theme: EditorTheme): void {
    themePreference.value = theme;
    applyThemeToDocument(theme);
  }

  function applySettingsStatus(status: SettingsLoadStatus): void {
    if (snapshot.value) {
      snapshot.value = {
        ...snapshot.value,
        status,
      };
    }
  }

  async function initialize(): Promise<void> {
    loading.value = true;
    try {
      const [settingsRes, uiStateRes] = await Promise.all([
        fetchSettingsSnapshot(),
        fetchUiState(),
      ]);
      snapshot.value = settingsRes;
      hotkeyStatus.value = uiStateRes.hotkey;
      setTheme(settingsRes.settings.editor.theme);

      if (!unlistenStatus) {
        unlistenStatus = await subscribeSettingsStatus((nextStatus) => {
          applySettingsStatus(nextStatus);
        });
      }
    } finally {
      loading.value = false;
    }
  }

  async function switchToBackupHotkey(): Promise<HotkeyStatus | null> {
    const targetCombo = hotkeyStatus.value?.backupCombo || BACKUP_HOTKEY_COMBO;
    updatingHotkey.value = true;
    hotkeyError.value = null;
    try {
      const nextStatus = await updateGlobalHotkey({ combo: targetCombo });
      hotkeyStatus.value = nextStatus;
      return nextStatus;
    } catch (error) {
      hotkeyError.value = extractSettingsErrorMessage(error);
      return null;
    } finally {
      updatingHotkey.value = false;
    }
  }

  function dispose(): void {
    if (unlistenStatus) {
      unlistenStatus();
      unlistenStatus = null;
    }
  }

  return {
    snapshot,
    hotkeyStatus,
    themePreference,
    activeSection,
    loading,
    updatingHotkey,
    hotkeyError,
    loadStatus,
    isSettingsInvalid,
    isHotkeyConflict,
    setTheme,
    applySettingsStatus,
    initialize,
    switchToBackupHotkey,
    dispose,
  };
});
