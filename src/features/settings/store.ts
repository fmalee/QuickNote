import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type {
  BackupPathsUiState,
  EditorTheme,
  HotkeyStatus,
  NoteDoc,
  NoteSummary,
  SettingsLoadStatus,
  SettingsPatch,
  SettingsSnapshot,
  WindowMode,
  WindowState,
} from '@/bindings/types';
import {
  pickBackupOpenPath,
  pickBackupSavePath,
  uiStateSet,
  windowShowMain,
} from '@/lib/ipc/commands';
import type { UnlistenFn } from '@/lib/ipc/events';
import { applyThemeToDocument } from '@/lib/utils';
import {
  changeLibraryRoot,
  createTemplateNote,
  exportSettingsBackup,
  extractSettingsErrorMessage,
  fetchSettingsSnapshot,
  fetchUiState,
  importSettingsBackup,
  listTemplateNotes,
  patchBackupPaths,
  saveSettingsPatch,
  subscribeSettingsStatus,
  subscribeWindowState,
  updateGlobalHotkey,
} from './api';

export type SettingsSection =
  'general' | 'appearance' | 'library' | 'workspaces' | 'external' | 'shortcuts' | 'baseline';

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
  const backupBusy = ref<boolean>(false);
  const backupError = ref<string | null>(null);
  const backupSuccess = ref<string | null>(null);
  const backupPaths = ref<BackupPathsUiState>({
    lastExportPath: null,
    lastImportPath: null,
  });
  const exportBackupPathDraft = ref<string>('');
  const importBackupPathDraft = ref<string>('');
  const mainWindowState = ref<WindowState | null>(null);

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
  const showTabCloseButton = computed<boolean>(
    () => snapshot.value?.settings.editor.showTabCloseButton ?? false,
  );

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

  function applyBackupPathsUiState(next?: BackupPathsUiState): void {
    if (!next) {
      return;
    }
    backupPaths.value = {
      lastExportPath: next.lastExportPath ?? null,
      lastImportPath: next.lastImportPath ?? null,
    };
    exportBackupPathDraft.value = backupPaths.value.lastExportPath ?? '';
    importBackupPathDraft.value = backupPaths.value.lastImportPath ?? '';
  }

  async function refreshUiState(): Promise<void> {
    try {
      const uiStateRes = await fetchUiState();
      hotkeyStatus.value = uiStateRes.hotkey;
      applyBackupPathsUiState(uiStateRes.backupPaths);
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
      applyBackupPathsUiState(uiStateRes.backupPaths);
      setTheme(settingsRes.settings.editor.theme);

      if (!unlistenStatus) {
        unlistenStatus = await subscribeSettingsStatus((nextStatus) => {
          void handleSettingsStatusEvent(nextStatus);
        });
      }
      if (!unlistenWindowState) {
        unlistenWindowState = await subscribeWindowState((winState) => {
          if (winState.windowId === 'main') {
            mainWindowState.value = winState;
          }
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

  async function exportSettings(destPath: string): Promise<boolean> {
    const trimmed = destPath.trim();
    if (!trimmed) {
      backupError.value = '导出路径不能为空。';
      backupSuccess.value = null;
      return false;
    }

    backupBusy.value = true;
    backupError.value = null;
    backupSuccess.value = null;
    try {
      await exportSettingsBackup(trimmed);
      backupPaths.value = {
        ...backupPaths.value,
        lastExportPath: trimmed,
      };
      exportBackupPathDraft.value = trimmed;
      try {
        await patchBackupPaths(backupPaths.value);
      } catch {
        // 非 Tauri 测试环境容错
      }
      backupSuccess.value = `配置备份已导出至 ${trimmed}`;
      return true;
    } catch (error) {
      backupError.value = extractSettingsErrorMessage(error);
      return false;
    } finally {
      backupBusy.value = false;
    }
  }

  async function importSettings(srcPath: string): Promise<boolean> {
    const trimmed = srcPath.trim();
    if (!trimmed) {
      backupError.value = '导入路径不能为空。';
      backupSuccess.value = null;
      return false;
    }

    backupBusy.value = true;
    backupError.value = null;
    backupSuccess.value = null;
    try {
      const imported = await importSettingsBackup(trimmed);
      snapshot.value = imported;
      applyThemeLocal(imported.settings.editor.theme);
      saveError.value = null;
      lastSavedAt.value = Date.now();
      await refreshUiState();
      backupPaths.value = {
        ...backupPaths.value,
        lastImportPath: trimmed,
      };
      importBackupPathDraft.value = trimmed;
      try {
        await patchBackupPaths(backupPaths.value);
      } catch {
        // 非 Tauri 测试环境容错
      }
      backupSuccess.value = '配置已成功导入并热更新生效。';
      return true;
    } catch (error) {
      backupError.value = extractSettingsErrorMessage(error);
      return false;
    } finally {
      backupBusy.value = false;
    }
  }

  async function pickAndExportSettings(): Promise<boolean> {
    const picked = await pickBackupSavePath(backupPaths.value.lastExportPath);
    if (!picked) {
      return false;
    }
    exportBackupPathDraft.value = picked;
    return exportSettings(picked);
  }

  async function pickAndImportSettings(): Promise<boolean> {
    const picked = await pickBackupOpenPath(backupPaths.value.lastImportPath);
    if (!picked) {
      return false;
    }
    importBackupPathDraft.value = picked;
    return importSettings(picked);
  }

  async function setMainWindowMode(mode: WindowMode): Promise<boolean> {
    const ok = await patchSettings({ mainWindow: { mode } });
    if (ok) {
      try {
        const currentUi = await fetchUiState();
        const prevWindows = currentUi.windows ?? {
          main: { x: 120, y: 80, width: 1200, height: 800, mode, isOpen: true },
        };
        await uiStateSet({
          key: 'windows',
          value: {
            ...prevWindows,
            main: {
              ...prevWindows.main,
              mode,
            },
          },
        });
      } catch {
        // 非 Tauri 测试环境容错
      }
    }
    return ok;
  }

  async function setAuxiliaryWidthPercent(percent: number): Promise<boolean> {
    const normalized = Number.isFinite(percent) ? Math.round(percent) : 45;
    const clamped = Math.min(60, Math.max(30, normalized));
    const ok = await patchSettings({ mainWindow: { auxiliaryWidthPercent: clamped } });
    if (ok && mainWindowState.value?.mode === 'auxiliary') {
      try {
        await windowShowMain({ mode: 'auxiliary' });
      } catch {
        // 非 Tauri 测试环境容错
      }
    }
    return ok;
  }

  async function setQuickBarDestroyMinutes(minutes: number): Promise<boolean> {
    const normalized = Number.isFinite(minutes) ? Math.round(minutes) : 10;
    const clamped = Math.max(0, normalized);
    return patchSettings({ quickBar: { destroyAfterHiddenMinutes: clamped } });
  }

  async function previewAuxiliaryWindow(): Promise<void> {
    try {
      const nextState = await windowShowMain({ mode: 'auxiliary' });
      mainWindowState.value = nextState;
    } catch {
      // 非 Tauri 测试环境容错
    }
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
    backupBusy,
    backupError,
    backupSuccess,
    backupPaths,
    exportBackupPathDraft,
    importBackupPathDraft,
    mainWindowState,
    loadStatus,
    isSettingsInvalid,
    isHotkeyConflict,
    isUsingBackupHotkey,
    showTabCloseButton,
    setTheme,
    patchSettings,
    setMainWindowMode,
    setAuxiliaryWidthPercent,
    setQuickBarDestroyMinutes,
    previewAuxiliaryWindow,
    exportSettings,
    importSettings,
    pickAndExportSettings,
    pickAndImportSettings,
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
