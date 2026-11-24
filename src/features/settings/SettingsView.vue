<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { WindowMode } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnInput, QnPanelCard } from '@/components';
import { formatHotkeyCombo } from '@/lib/utils';
import SettingsAppearancePanel from './SettingsAppearancePanel.vue';
import SettingsErrorBanner from './SettingsErrorBanner.vue';
import SettingsExternalPanel from './SettingsExternalPanel.vue';
import SettingsLibraryPanel from './SettingsLibraryPanel.vue';
import SettingsShortcutsPanel from './SettingsShortcutsPanel.vue';
import SettingsWorkspacesPanel from './SettingsWorkspacesPanel.vue';
import VisualBaselinePreview from './VisualBaselinePreview.vue';
import { BACKUP_HOTKEY_COMBO, type SettingsSection, useSettingsStore } from './store';

const store = useSettingsStore();
const exportPathDraft = ref('~/Desktop/quicknote-settings-backup.yaml');
const importPathDraft = ref('');
const auxiliaryWidthDraft = ref<number>(
  store.snapshot?.settings.mainWindow.auxiliaryWidthPercent ?? 45,
);
const quickBarDestroyDraft = ref<number>(
  store.snapshot?.settings.quickBar.destroyAfterHiddenMinutes ?? 10,
);

watch(
  () => store.snapshot?.settings,
  (nextSettings) => {
    if (!nextSettings) {
      return;
    }
    auxiliaryWidthDraft.value = nextSettings.mainWindow.auxiliaryWidthPercent;
    quickBarDestroyDraft.value = nextSettings.quickBar.destroyAfterHiddenMinutes;
  },
  { immediate: true },
);

async function handleMainWindowModeChange(event: Event): Promise<void> {
  const target = event.target as HTMLSelectElement | null;
  if (!target) {
    return;
  }
  const mode: WindowMode = target.value === 'auxiliary' ? 'auxiliary' : 'standard';
  await store.setMainWindowMode(mode);
}

async function handleAuxiliaryWidthChange(): Promise<void> {
  await store.setAuxiliaryWidthPercent(Number(auxiliaryWidthDraft.value));
  if (store.snapshot?.settings) {
    auxiliaryWidthDraft.value = store.snapshot.settings.mainWindow.auxiliaryWidthPercent;
  }
}

async function handleQuickBarDestroyChange(): Promise<void> {
  await store.setQuickBarDestroyMinutes(Number(quickBarDestroyDraft.value));
  if (store.snapshot?.settings) {
    quickBarDestroyDraft.value = store.snapshot.settings.quickBar.destroyAfterHiddenMinutes;
  }
}

async function handleExportSettings(): Promise<void> {
  await store.exportSettings(exportPathDraft.value);
}

async function handleImportSettings(): Promise<void> {
  await store.importSettings(importPathDraft.value);
}

const sections: { id: SettingsSection; label: string }[] = [
  { id: 'general', label: '通用与热键' },
  { id: 'appearance', label: '外观与编辑器' },
  { id: 'library', label: '库与收件箱' },
  { id: 'workspaces', label: '工作区' },
  { id: 'external', label: '外部与搜索' },
  { id: 'shortcuts', label: '快捷键' },
  { id: 'baseline', label: '视觉基线预览' },
];

const saveStatusLabel = computed<string | null>(() => {
  if (store.saving) {
    return '正在保存…';
  }
  if (store.lastSavedAt) {
    return '已保存';
  }
  return null;
});

const selfCheckBadge = computed<{ tone: 'success' | 'warning' | 'danger'; label: string }>(() => {
  const status = store.hotkeyStatus?.selfCheck ?? 'pending';
  if (status === 'ok') {
    return { tone: 'success', label: '自检通过 (ok)' };
  }
  if (status === 'conflict') {
    return { tone: 'danger', label: '热键冲突 (conflict)' };
  }
  return { tone: 'warning', label: '待自检 (pending)' };
});
</script>

<template>
  <div class="qn-settings">
    <aside class="qn-settings__sidebar">
      <div class="qn-settings__sidebar-title">QuickNote 设置</div>
      <nav class="qn-settings__nav">
        <button
          v-for="item in sections"
          :key="item.id"
          type="button"
          :data-testid="`settings-nav-${item.id}`"
          :class="[
            'qn-settings__nav-item',
            { 'qn-settings__nav-item--active': store.activeSection === item.id },
          ]"
          @click="store.activeSection = item.id"
        >
          {{ item.label }}
        </button>
      </nav>
    </aside>

    <main class="qn-settings__content">
      <div v-if="saveStatusLabel" class="qn-settings__save-bar">
        <QnBadge :tone="store.saving ? 'warning' : 'success'" data-testid="settings-save-status">
          {{ saveStatusLabel }}
        </QnBadge>
      </div>

      <SettingsErrorBanner :status="store.loadStatus" />

      <QnBanner
        v-if="store.saveError"
        tone="danger"
        title="保存设置失败"
        data-testid="settings-save-error-banner"
      >
        {{ store.saveError }}
      </QnBanner>

      <section v-if="store.activeSection === 'general'" class="qn-settings__section">
        <QnPanelCard
          title="全局热键状态"
          subtitle="呼出 Quick Bar 浮层（D-0004：首启自检与热重注册）"
          data-testid="hotkey-status-card"
        >
          <template #header>
            <QnBadge :tone="selfCheckBadge.tone">{{ selfCheckBadge.label }}</QnBadge>
          </template>

          <div class="qn-settings__hotkey-body">
            <QnBanner
              v-if="store.isHotkeyConflict"
              tone="warning"
              title="检测到全局热键冲突"
              data-testid="hotkey-conflict-banner"
            >
              当前热键
              {{ formatHotkeyCombo(store.hotkeyStatus?.combo ?? 'ctrl+alt+space') }}
              可能已被系统或输入法占用，建议{{
                store.isUsingBackupHotkey ? '切回默认主热键' : '一键切换至备用热键'
              }}。
              <template #actions>
                <QnButton
                  v-if="store.isUsingBackupHotkey"
                  variant="primary"
                  size="sm"
                  :loading="store.updatingHotkey"
                  data-testid="switch-primary-hotkey-btn"
                  @click="store.switchToPrimaryHotkey()"
                >
                  一键切回主热键 ⌃⌥Space
                </QnButton>
                <QnButton
                  v-else
                  variant="primary"
                  size="sm"
                  :loading="store.updatingHotkey"
                  data-testid="switch-backup-hotkey-btn"
                  @click="store.switchToBackupHotkey()"
                >
                  一键切至备用热键 ⌃⌥⌘Space
                </QnButton>
              </template>
            </QnBanner>

            <QnBanner v-if="store.hotkeyError" tone="danger" title="热键更新失败">
              {{ store.hotkeyError }}
            </QnBanner>

            <div class="qn-settings__kv-list">
              <div class="qn-settings__kv-row">
                <span class="qn-settings__kv-label">当前主热键</span>
                <code class="qn-settings__kv-value">
                  {{ formatHotkeyCombo(store.hotkeyStatus?.combo ?? 'ctrl+alt+space') }}
                  ({{ store.hotkeyStatus?.combo ?? 'ctrl+alt+space' }})
                </code>
              </div>
              <div class="qn-settings__kv-row">
                <span class="qn-settings__kv-label">备用热键</span>
                <code class="qn-settings__kv-value">
                  {{ formatHotkeyCombo(store.hotkeyStatus?.backupCombo ?? BACKUP_HOTKEY_COMBO) }}
                  ({{ store.hotkeyStatus?.backupCombo ?? BACKUP_HOTKEY_COMBO }})
                </code>
              </div>
            </div>

            <div v-if="!store.isHotkeyConflict" class="qn-settings__hotkey-actions">
              <QnButton
                v-if="store.isUsingBackupHotkey"
                variant="secondary"
                size="sm"
                :loading="store.updatingHotkey"
                data-testid="switch-primary-hotkey-btn"
                @click="store.switchToPrimaryHotkey()"
              >
                切回默认主热键 ⌃⌥Space
              </QnButton>
              <QnButton
                v-else
                variant="secondary"
                size="sm"
                :loading="store.updatingHotkey"
                data-testid="switch-backup-hotkey-btn"
                @click="store.switchToBackupHotkey()"
              >
                切换至备用热键 ⌃⌥⌘Space
              </QnButton>
            </div>
          </div>
        </QnPanelCard>

        <QnPanelCard
          title="主窗口形态与内存回收"
          subtitle="默认窗口形态、45% 侧栏辅助比例、全屏引导与 Quick Bar 回收策略"
          data-testid="settings-window-mode-card"
        >
          <div class="qn-settings__window-mode-stack">
            <div class="qn-settings__kv-row">
              <div class="qn-settings__field-meta">
                <span class="qn-settings__kv-label">默认窗口模式</span>
                <span class="qn-settings__field-hint">
                  45% 侧栏辅助默认关闭，适合全屏阅读对照记录
                </span>
              </div>
              <div class="qn-settings__inline-controls">
                <select
                  class="qn-settings__select"
                  :value="
                    store.snapshot?.settings.mainWindow.mode === 'auxiliary'
                      ? 'auxiliary'
                      : 'standard'
                  "
                  :disabled="store.saving || store.isSettingsInvalid"
                  data-testid="settings-main-window-mode-select"
                  @change="handleMainWindowModeChange"
                >
                  <option value="standard">标准窗口（默认）</option>
                  <option value="auxiliary">45% 侧栏辅助</option>
                </select>
                <QnButton
                  variant="secondary"
                  size="sm"
                  data-testid="preview-auxiliary-window-btn"
                  @click="store.previewAuxiliaryWindow()"
                >
                  立即以侧栏辅助模式显示主窗口
                </QnButton>
              </div>
            </div>

            <div class="qn-settings__kv-row">
              <div class="qn-settings__field-meta">
                <span class="qn-settings__kv-label">侧栏辅助宽度比例（%）</span>
                <span class="qn-settings__field-hint">范围 30%–60%，默认 45%</span>
              </div>
              <input
                v-model.number="auxiliaryWidthDraft"
                type="number"
                min="30"
                max="60"
                class="qn-settings__number-input"
                :disabled="store.saving || store.isSettingsInvalid"
                data-testid="settings-auxiliary-width-input"
                @change="handleAuxiliaryWidthChange"
              />
            </div>

            <div class="qn-settings__kv-row">
              <div class="qn-settings__field-meta">
                <span class="qn-settings__kv-label">Quick Bar 隐藏后内存释放延时（分钟）</span>
                <span class="qn-settings__field-hint">默认 10 分钟，设为 0 表示不自动销毁</span>
              </div>
              <input
                v-model.number="quickBarDestroyDraft"
                type="number"
                min="0"
                class="qn-settings__number-input"
                :disabled="store.saving || store.isSettingsInvalid"
                data-testid="settings-quickbar-destroy-input"
                @change="handleQuickBarDestroyChange"
              />
            </div>

            <QnBanner
              tone="info"
              title="全屏工作台 Mission Control 排序提示"
              data-testid="fullscreen-mission-control-tip"
            >
              按 ⌃⌘F 进入全屏工作台后，可在 Mission Control（⌃↑）中一次性将 QuickNote
              空间拖至主工作应用左侧，macOS 将自动持久化排序。
            </QnBanner>
          </div>
        </QnPanelCard>

        <QnPanelCard title="配置文件与笔记库" subtitle="settings.yaml 单一配置源（D-0008）">
          <div class="qn-settings__kv-list">
            <div class="qn-settings__kv-row">
              <span class="qn-settings__kv-label">笔记库根目录</span>
              <code class="qn-settings__kv-value">
                {{ store.snapshot?.settings.libraryRoot ?? '未配置（等待初始化）' }}
              </code>
            </div>
            <div class="qn-settings__kv-row">
              <span class="qn-settings__kv-label">当前生效配置源</span>
              <code class="qn-settings__kv-value">
                {{ store.loadStatus?.activeSource ?? 'default' }}
              </code>
            </div>
          </div>
        </QnPanelCard>

        <QnPanelCard
          title="配置备份与恢复"
          subtitle="导出 settings.yaml 备份或从已有 YAML 文件恢复配置"
          data-testid="settings-backup-card"
        >
          <div class="qn-settings__backup-stack">
            <div class="qn-settings__backup-row">
              <QnInput
                v-model="exportPathDraft"
                label="导出备份路径"
                placeholder="~/Desktop/quicknote-settings-backup.yaml"
                monospace
                :disabled="store.backupBusy"
                data-testid="settings-export-path-input"
              />
              <QnButton
                variant="secondary"
                size="sm"
                :loading="store.backupBusy"
                data-testid="settings-export-btn"
                @click="handleExportSettings"
              >
                导出配置备份
              </QnButton>
            </div>

            <div class="qn-settings__backup-row">
              <QnInput
                v-model="importPathDraft"
                label="导入备份路径"
                placeholder="~/Desktop/quicknote-settings-backup.yaml"
                monospace
                :disabled="store.backupBusy"
                data-testid="settings-import-path-input"
              />
              <QnButton
                variant="primary"
                size="sm"
                :loading="store.backupBusy"
                data-testid="settings-import-btn"
                @click="handleImportSettings"
              >
                导入并恢复配置
              </QnButton>
            </div>

            <p class="qn-settings__backup-hint">
              导入前将自动备份当前配置至 backups/，格式错误不会覆盖现有文件
            </p>

            <QnBanner
              v-if="store.backupError"
              tone="danger"
              title="备份操作失败"
              data-testid="settings-backup-error"
            >
              {{ store.backupError }}
            </QnBanner>

            <QnBanner
              v-if="store.backupSuccess"
              tone="success"
              title="操作成功"
              data-testid="settings-backup-success"
            >
              {{ store.backupSuccess }}
            </QnBanner>
          </div>
        </QnPanelCard>
      </section>

      <section v-else-if="store.activeSection === 'appearance'" class="qn-settings__section">
        <SettingsAppearancePanel />
      </section>

      <section v-else-if="store.activeSection === 'library'" class="qn-settings__section">
        <SettingsLibraryPanel />
      </section>

      <section v-else-if="store.activeSection === 'workspaces'" class="qn-settings__section">
        <SettingsWorkspacesPanel />
      </section>

      <section v-else-if="store.activeSection === 'external'" class="qn-settings__section">
        <SettingsExternalPanel />
      </section>

      <section v-else-if="store.activeSection === 'shortcuts'" class="qn-settings__section">
        <SettingsShortcutsPanel />
      </section>

      <section v-else class="qn-settings__section">
        <VisualBaselinePreview />
      </section>
    </main>
  </div>
</template>

<style scoped>
.qn-settings {
  display: flex;
  width: 100%;
  height: 100%;
  background-color: var(--qn-bg-base);
}

.qn-settings__sidebar {
  width: 210px;
  flex-shrink: 0;
  padding: var(--qn-space-4);
  background-color: var(--qn-bg-sidebar);
  border-right: 1px solid var(--qn-border-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-settings__sidebar-title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-muted);
  padding: 0 var(--qn-space-2);
}

.qn-settings__nav {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-settings__nav-item {
  width: 100%;
  text-align: left;
  padding: var(--qn-space-2) var(--qn-space-3);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-sm);
  font-weight: 500;
  cursor: pointer;
}

.qn-settings__nav-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-settings__nav-item--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-settings__content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: var(--qn-space-6);
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-settings__section {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-settings__hotkey-body {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-settings__kv-list {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-settings__kv-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-4);
  padding: var(--qn-space-2) 0;
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-settings__kv-row:last-child {
  border-bottom: none;
}

.qn-settings__kv-label {
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-settings__kv-value {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
}

.qn-settings__hotkey-actions {
  display: flex;
  justify-content: flex-end;
}

.qn-settings__save-bar {
  display: flex;
  justify-content: flex-end;
}

.qn-settings__backup-stack {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-settings__backup-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: var(--qn-space-3);
}

.qn-settings__backup-hint {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-settings__window-mode-stack {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-settings__field-meta {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-settings__field-hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-settings__inline-controls {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-wrap: wrap;
}

.qn-settings__select,
.qn-settings__number-input {
  height: 32px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-default);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-surface);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-settings__number-input {
  width: 88px;
  text-align: right;
}
</style>
