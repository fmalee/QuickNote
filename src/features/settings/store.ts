import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  EditorTheme,
  HotkeyStatus,
  NoteDoc,
  NoteSummary,
  SettingsLoadStatus,
  SettingsPatch,
  SettingsSnapshot,
} from '@/bindings/types';
import type { UnlistenFn } from '@/lib/ipc/events';
import { applyThemeToDocument } from '@/lib/utils';
import {
  changeLibraryRoot,
  createTemplateNote,
  extractSettingsErrorMessage,
  fetchSettingsSnapshot,
  fetchUiState,
  listTemplateNotes,
  saveSettingsPatch,
  subscribeSettingsStatus,
  subscribeWindowState,
  updateGlobalHotkey,
} from './api';

export type SettingsSection = 'general' | 'appearance' | 'library' | 'shortcuts' | 'baseline';

export const PRIMARY_HOTKEY_COMBO = 'ctrl+alt+space';
export const BACKUP_HOTKEY_COMBO = 'ctrl+alt+cmd+space';
export const SETTINGS_READONLY_ERROR =
  'settings.yaml 当前存在语法错误，已只读保护，请先修复原文件后再保存';

export const useSettingsStore = defineStore('settings', () => {
  const snapshot = ref<SettingsSnapshot | null>(null);
  const hotkeyStatus = ref<HotkeyStatus | null>(null);
  const themePreference = ref<EditorTheme>('system');
  const activeSection = ref<SettingsSection>('general');
  const loading = ref<boolean>(false);
  const saving = ref<boolean>(false);
  const saveError = ref<string | null>(null);
  const lastSavedAt = ref<number | null>(null);
  const updatingHotkey = ref<boolean>(false);
  const hotkeyError = ref<string | null>(null);
  const templates = ref<NoteSummary[]>([]);
  const loadingTemplates = ref<boolean>(false);

  let unlistenStatus: UnlistenFn | null = null;
  let unlistenWindowState: UnlistenFn | null = null;

  const loadStatus = computed<SettingsLoadStatus | null>(() => snapshot.value?.status ?? null);
  const isSettingsInvalid = computed<boolean>(
    () => loadStatus.value !== null && !loadStatus.value.valid,
  );
  const isHotkeyConflict = computed<boolean>(() => hotkeyStatus.value?.selfCheck === 'conflict');
  const isUsingBackupHotkey = computed<boolean>(() => {
    const active = hotkeyStatus.value?.combo;
    const backup = hotkeyStatus.value?.backupCombo || BACKUP_HOTKEY_COMBO;
    return active === backup;
  });

  function applyThemeLocal(theme: EditorTheme): void {
    themePreference.value = theme;
    applyThemeToDocument(theme);
  }

  function setTheme(theme: EditorTheme, options?: { persist?: boolean }): void {
    applyThemeLocal(theme);
    if (options?.persist) {
      void patchSettings({ editor: { theme } });
    }
  }

  async function patchSettings(patch: SettingsPatch): Promise<boolean> {
    if (isSettingsInvalid.value) {
      saveError.value = SETTINGS_READONLY_ERROR;
      return false;
    }

    saving.value = true;
    saveError.value = null;
    try {
      const updated = await saveSettingsPatch(patch);
      snapshot.value = updated;
      applyThemeLocal(updated.settings.editor.theme);
      lastSavedAt.value = Date.now();
      return true;
    } catch (error) {
      saveError.value = extractSettingsErrorMessage(error);
      return false;
    } finally {
      saving.value = false;
    }
  }

  async function loadTemplates(): Promise<NoteSummary[]> {
    loadingTemplates.value = true;
    try {
      const list = await listTemplateNotes();
      templates.value = list;
      return list;
    } catch (error) {
      saveError.value = extractSettingsErrorMessage(error);
      return templates.value;
    } finally {
      loadingTemplates.value = false;
    }
  }

  async function addTemplate(title: string): Promise<NoteDoc | null> {
    const trimmed = title.trim();
    if (!trimmed) {
      saveError.value = '模板名称不能为空。';
      return null;
    }
    saving.value = true;
    saveError.value = null;
    try {
      const created = await createTemplateNote(trimmed);
      await loadTemplates();
      lastSavedAt.value = Date.now();
      return created;
    } catch (error) {
      saveError.value = extractSettingsErrorMessage(error);
      return null;
    } finally {
      saving.value = false;
    }
  }

  async function updateLibraryRoot(root: string, createIfMissing: boolean): Promise<boolean> {
    if (isSettingsInvalid.value) {
      saveError.value = SETTINGS_READONLY_ERROR;
      return false;
    }
    const trimmed = root.trim();
    if (!trimmed) {
      saveError.value = '笔记库路径不能为空。';
      return false;
    }

    saving.value = true;
    saveError.value = null;
    try {
      await changeLibraryRoot({ root: trimmed, createIfMissing });
      const refreshed = await fetchSettingsSnapshot();
      snapshot.value = refreshed;
      applyThemeLocal(refreshed.settings.editor.theme);
      await loadTemplates();
      lastSavedAt.value = Date.now();
      return true;
    } catch (error) {
      saveError.value = extractSettingsErrorMessage(error);
      return false;
    } finally {
      saving.value = false;
    }
  }

  function applySettingsStatus(status: SettingsLoadStatus): void {
    if (snapshot.value) {
      snapshot.value = {
        ...snapshot.value,
        status,
      };
    }
  }

  async function handleSettingsStatusEvent(status: SettingsLoadStatus): Promise<void> {
    applySettingsStatus(status);
    if (!status.valid) {
      return;
    }
    try {
      const refreshed = await fetchSettingsSnapshot();
      snapshot.value = refreshed;
      setTheme(refreshed.settings.editor.theme);
    } catch (error) {
      console.error(
        '[settings] failed to refresh settings snapshot after status event:',
        extractSettingsErrorMessage(error),
      );
    }
  }

  async function refreshUiState(): Promise<void> {
    try {
      const uiStateRes = await fetchUiState();
      hotkeyStatus.value = uiStateRes.hotkey;
    } catch (error) {
      console.error('[settings] failed to refresh ui-state:', extractSettingsErrorMessage(error));
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
          void handleSettingsStatusEvent(nextStatus);
        });
      }
      if (!unlistenWindowState) {
        unlistenWindowState = await subscribeWindowState((winState) => {
          if (
            winState.windowId === 'quickbar' ||
            (winState.windowId === 'settings' && winState.visible)
          ) {
            void refreshUiState();
          }
        });
      }
    } finally {
      loading.value = false;
    }
  }

  async function switchHotkey(targetCombo: string): Promise<HotkeyStatus | null> {
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

  function switchToBackupHotkey(): Promise<HotkeyStatus | null> {
    const targetCombo = hotkeyStatus.value?.backupCombo || BACKUP_HOTKEY_COMBO;
    return switchHotkey(targetCombo);
  }

  function switchToPrimaryHotkey(): Promise<HotkeyStatus | null> {
    return switchHotkey(PRIMARY_HOTKEY_COMBO);
  }

  function dispose(): void {
    if (unlistenStatus) {
      unlistenStatus();
      unlistenStatus = null;
    }
    if (unlistenWindowState) {
      unlistenWindowState();
      unlistenWindowState = null;
    }
  }

  return {
    snapshot,
    hotkeyStatus,
    themePreference,
    activeSection,
    loading,
    saving,
    saveError,
    lastSavedAt,
    updatingHotkey,
    hotkeyError,
    templates,
    loadingTemplates,
    loadStatus,
    isSettingsInvalid,
    isHotkeyConflict,
    isUsingBackupHotkey,
    setTheme,
    patchSettings,
    loadTemplates,
    addTemplate,
    updateLibraryRoot,
    applySettingsStatus,
    refreshUiState,
    initialize,
    switchToBackupHotkey,
    switchToPrimaryHotkey,
    dispose,
  };
});
