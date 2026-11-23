<script setup lang="ts">
import { computed } from 'vue';
import type { EditorTheme } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnPanelCard } from '@/components';
import { formatHotkeyCombo } from '@/lib/utils';
import SettingsErrorBanner from './SettingsErrorBanner.vue';
import VisualBaselinePreview from './VisualBaselinePreview.vue';
import { BACKUP_HOTKEY_COMBO, type SettingsSection, useSettingsStore } from './store';

const store = useSettingsStore();

const sections: { id: SettingsSection; label: string }[] = [
  { id: 'general', label: '通用与快捷键' },
  { id: 'appearance', label: '外观主题' },
  { id: 'baseline', label: '视觉基线预览' },
];

const themeOptions: { value: EditorTheme; label: string; desc: string }[] = [
  { value: 'system', label: '跟随系统', desc: '自动匹配 macOS 深浅色外观' },
  { value: 'light', label: '浅色主题', desc: '高对比日间纸面阅读体验' },
  { value: 'dark', label: '深色主题', desc: '低眩光夜间专注工作模式' },
];

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
      <SettingsErrorBanner :status="store.loadStatus" />

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
      </section>

      <section v-else-if="store.activeSection === 'appearance'" class="qn-settings__section">
        <QnPanelCard title="主题模式" subtitle="实时切换 --qn-* 浅色与深色视觉变量">
          <div class="qn-settings__theme-grid">
            <button
              v-for="opt in themeOptions"
              :key="opt.value"
              type="button"
              :class="[
                'qn-settings__theme-card',
                { 'qn-settings__theme-card--active': store.themePreference === opt.value },
              ]"
              @click="store.setTheme(opt.value)"
            >
              <span class="qn-settings__theme-name">{{ opt.label }}</span>
              <span class="qn-settings__theme-desc">{{ opt.desc }}</span>
            </button>
          </div>
          <div class="qn-settings__appearance-footer">
            <QnButton variant="secondary" size="sm" @click="store.activeSection = 'baseline'">
              打开视觉基线预览页
            </QnButton>
          </div>
        </QnPanelCard>
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

.qn-settings__theme-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--qn-space-3);
}

.qn-settings__theme-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--qn-space-1);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
  cursor: pointer;
  text-align: left;
}

.qn-settings__theme-card:hover {
  background-color: var(--qn-bg-hover);
}

.qn-settings__theme-card--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-accent-soft);
}

.qn-settings__theme-name {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-settings__theme-desc {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-settings__appearance-footer {
  margin-top: var(--qn-space-4);
  display: flex;
  justify-content: flex-end;
}
</style>
