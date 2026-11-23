<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import { QnBadge, QnBanner, QnButton, QnInput, QnModalShell, QnPanelCard } from '@/components';
import {
  actionRegistry,
  formatKeyComboDisplay,
  keyboardEventToCombo,
  type ActionCategory,
  type ActionDef,
  type ShortcutConflict,
} from '@/lib/actions';

const searchQuery = ref('');
const selectedCategory = ref<'all' | ActionCategory>('all');
const recordingActionId = ref<string | null>(null);
const conflicts = ref<ShortcutConflict[]>([]);
const jsonModalOpen = ref(false);
const jsonModalMode = ref<'export' | 'import'>('export');
const jsonPayload = ref('');
const jsonFeedback = ref<string | null>(null);
const registryVersion = ref(0);

const categoryLabels: Record<'all' | ActionCategory, string> = {
  all: '全部',
  global: '全局',
  quickbar: 'Quick Bar',
  window: '窗口',
  editor: '编辑器',
  external: '外部打开',
  panel: '面板',
};

const filteredActions = computed<ActionDef[]>(() => {
  void registryVersion.value;
  const query = searchQuery.value.trim().toLowerCase();
  return actionRegistry.getAll().filter((action) => {
    if (selectedCategory.value !== 'all' && action.category !== selectedCategory.value) {
      return false;
    }
    if (!query) {
      return true;
    }
    const effective = actionRegistry.getEffectiveKeys(action.id) ?? '';
    return (
      action.title.toLowerCase().includes(query) ||
      action.id.toLowerCase().includes(query) ||
      action.scope.toLowerCase().includes(query) ||
      effective.toLowerCase().includes(query)
    );
  });
});

function getDisplayCombo(actionId: string): string {
  void registryVersion.value;
  const effective = actionRegistry.getEffectiveKeys(actionId);
  return effective ? formatKeyComboDisplay(effective) : '未绑定';
}

function isOverridden(action: ActionDef): boolean {
  void registryVersion.value;
  return actionRegistry.getEffectiveKeys(action.id) !== action.defaultKeys;
}

function stopRecording(): void {
  recordingActionId.value = null;
  if (typeof window !== 'undefined') {
    window.removeEventListener('keydown', handleRecordKeydown, true);
  }
}

function handleRecordKeydown(event: KeyboardEvent): void {
  const targetId = recordingActionId.value;
  if (!targetId) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();

  if (event.code === 'Escape' || event.key === 'Escape') {
    stopRecording();
    return;
  }

  if (event.code === 'Backspace' || event.key === 'Backspace') {
    actionRegistry.setKeyOverride(targetId, null);
    conflicts.value = [];
    registryVersion.value += 1;
    stopRecording();
    return;
  }

  const combo = keyboardEventToCombo(event);
  if (!combo) {
    return;
  }

  const result = actionRegistry.setKeyOverride(targetId, combo);
  if (!result.ok) {
    conflicts.value = result.conflicts;
    stopRecording();
    return;
  }

  conflicts.value = [];
  registryVersion.value += 1;
  stopRecording();
}

function startRecording(actionId: string): void {
  conflicts.value = [];
  if (recordingActionId.value === actionId) {
    stopRecording();
    return;
  }
  stopRecording();
  recordingActionId.value = actionId;
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleRecordKeydown, true);
  }
}

function resetSingle(actionId: string): void {
  actionRegistry.resetKeyOverride(actionId);
  conflicts.value = [];
  registryVersion.value += 1;
}

function resetAll(): void {
  actionRegistry.resetAllOverrides();
  conflicts.value = [];
  registryVersion.value += 1;
}

function openExportModal(): void {
  jsonModalMode.value = 'export';
  jsonPayload.value = actionRegistry.exportOverridesJson();
  jsonFeedback.value = null;
  jsonModalOpen.value = true;
}

function openImportModal(): void {
  jsonModalMode.value = 'import';
  jsonPayload.value = '';
  jsonFeedback.value = null;
  jsonModalOpen.value = true;
}

function applyImportJson(): void {
  const res = actionRegistry.importOverridesJson(jsonPayload.value);
  if (!res.ok) {
    jsonFeedback.value = res.error;
    if (res.conflicts) {
      conflicts.value = res.conflicts;
    }
    return;
  }
  conflicts.value = [];
  registryVersion.value += 1;
  jsonModalOpen.value = false;
}

onBeforeUnmount(() => {
  stopRecording();
});

defineExpose({
  handleRecordKeydown,
  startRecording,
  stopRecording,
});
</script>

<template>
  <div class="qn-shortcuts-panel">
    <QnPanelCard title="动作快捷键" subtitle="点击录制自定义组合键，按作用域自动检测冲突">
      <template #header>
        <div class="qn-shortcuts-panel__header-actions">
          <QnButton
            variant="secondary"
            size="sm"
            data-testid="shortcuts-export-btn"
            @click="openExportModal"
          >
            导出配置
          </QnButton>
          <QnButton
            variant="secondary"
            size="sm"
            data-testid="shortcuts-import-btn"
            @click="openImportModal"
          >
            导入配置
          </QnButton>
          <QnButton
            variant="ghost"
            size="sm"
            data-testid="shortcuts-reset-all-btn"
            @click="resetAll"
          >
            全部恢复默认
          </QnButton>
        </div>
      </template>

      <div class="qn-shortcuts-panel__toolbar">
        <QnInput
          v-model="searchQuery"
          placeholder="搜索动作名称、ID、作用域或快捷键…"
          data-testid="shortcuts-search-input"
        />
        <select
          v-model="selectedCategory"
          class="qn-shortcuts-panel__select"
          data-testid="shortcuts-category-select"
        >
          <option v-for="(label, key) in categoryLabels" :key="key" :value="key">
            {{ label }}
          </option>
        </select>
      </div>

      <QnBanner
        v-if="conflicts.length > 0"
        tone="danger"
        title="快捷键冲突，已拒绝覆盖"
        data-testid="shortcut-conflict-banner"
      >
        <ul class="qn-shortcuts-panel__conflict-list">
          <li v-for="item in conflicts" :key="`${item.actionId}-${item.scope}`">
            与动作「{{ item.title }}」（<code>{{ item.actionId }}</code
            >，作用域：{{ item.scope }}）的
            <code>{{ formatKeyComboDisplay(item.keys) }}</code> 冲突，原键位已保留。
          </li>
        </ul>
      </QnBanner>

      <div class="qn-shortcuts-panel__list">
        <div
          v-for="action in filteredActions"
          :key="action.id"
          class="qn-shortcuts-panel__row"
          :data-testid="`shortcut-row-${action.id}`"
        >
          <div class="qn-shortcuts-panel__info">
            <div class="qn-shortcuts-panel__title-line">
              <span class="qn-shortcuts-panel__title">{{ action.title }}</span>
              <QnBadge tone="neutral">{{ action.scope }}</QnBadge>
              <QnBadge v-if="isOverridden(action)" tone="accent">已自定义</QnBadge>
            </div>
            <code class="qn-shortcuts-panel__id">{{ action.id }}</code>
          </div>

          <div class="qn-shortcuts-panel__controls">
            <span
              v-if="recordingActionId === action.id"
              class="qn-shortcuts-panel__recording-hint"
              data-testid="shortcut-recording-hint"
            >
              请按下组合键，Esc 取消，Backspace 清除
            </span>
            <kbd v-else class="qn-shortcuts-panel__kbd">{{ getDisplayCombo(action.id) }}</kbd>

            <QnButton
              :variant="recordingActionId === action.id ? 'primary' : 'secondary'"
              size="sm"
              :data-testid="`record-shortcut-${action.id}`"
              @click="startRecording(action.id)"
            >
              {{ recordingActionId === action.id ? '取消录制' : '录制' }}
            </QnButton>

            <QnButton
              v-if="isOverridden(action)"
              variant="ghost"
              size="sm"
              :data-testid="`reset-shortcut-${action.id}`"
              @click="resetSingle(action.id)"
            >
              恢复默认
            </QnButton>
          </div>
        </div>
      </div>
    </QnPanelCard>

    <QnModalShell
      :open="jsonModalOpen"
      :title="jsonModalMode === 'export' ? '导出快捷键配置 JSON' : '导入快捷键配置 JSON'"
      max-width="md"
      @close="jsonModalOpen = false"
    >
      <div class="qn-shortcuts-panel__modal-body">
        <textarea
          v-model="jsonPayload"
          class="qn-shortcuts-panel__textarea"
          :readonly="jsonModalMode === 'export'"
          rows="10"
          placeholder='{"overrides": {"editor.italic": "ctrl+shift+i"}}'
          data-testid="shortcuts-json-textarea"
        />
        <p v-if="jsonFeedback" class="qn-shortcuts-panel__error">{{ jsonFeedback }}</p>
      </div>
      <template #footer>
        <div class="qn-shortcuts-panel__modal-footer">
          <QnButton variant="ghost" size="sm" @click="jsonModalOpen = false">关闭</QnButton>
          <QnButton
            v-if="jsonModalMode === 'import'"
            variant="primary"
            size="sm"
            data-testid="shortcuts-import-confirm-btn"
            @click="applyImportJson"
          >
            应用导入
          </QnButton>
        </div>
      </template>
    </QnModalShell>
  </div>
</template>

<style scoped>
.qn-shortcuts-panel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-shortcuts-panel__header-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-shortcuts-panel__toolbar {
  display: grid;
  grid-template-columns: 1fr 140px;
  gap: var(--qn-space-2);
  margin-bottom: var(--qn-space-3);
}

.qn-shortcuts-panel__select {
  height: 36px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-shortcuts-panel__conflict-list {
  margin: 0;
  padding-left: var(--qn-space-4);
}

.qn-shortcuts-panel__list {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  margin-top: var(--qn-space-3);
}

.qn-shortcuts-panel__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-shortcuts-panel__info {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  min-width: 0;
}

.qn-shortcuts-panel__title-line {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-shortcuts-panel__title {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
}

.qn-shortcuts-panel__id {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-shortcuts-panel__controls {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}

.qn-shortcuts-panel__kbd {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
}

.qn-shortcuts-panel__recording-hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-accent);
  font-weight: 500;
}

.qn-shortcuts-panel__modal-body {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-shortcuts-panel__textarea {
  width: 100%;
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
}

.qn-shortcuts-panel__error {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}

.qn-shortcuts-panel__modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--qn-space-2);
}
</style>
