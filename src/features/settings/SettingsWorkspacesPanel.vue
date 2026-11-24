<script setup lang="ts">
import { computed, reactive, ref } from 'vue';
import type { WorkspaceConfig } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnInput, QnPanelCard } from '@/components';
import { SETTINGS_READONLY_ERROR, useSettingsStore } from './store';

const store = useSettingsStore();

const editingOriginalKey = ref<string | null>(null);
const formOpen = ref(false);
const openDirTouched = ref(false);
const formError = ref<string | null>(null);

const form = reactive({
  key: '',
  name: '',
  root: '',
  openDir: '',
  codeWorkspace: '',
  terminalDir: '',
  defaultEditor: 'vscode',
  defaultEntryDoc: '',
});

const workspaceList = computed<WorkspaceConfig[]>(() => {
  const map = store.snapshot?.settings.workspaces ?? {};
  return Object.values(map).sort((a, b) => a.key.localeCompare(b.key));
});

function resetForm(): void {
  editingOriginalKey.value = null;
  openDirTouched.value = false;
  formError.value = null;
  form.key = '';
  form.name = '';
  form.root = '';
  form.openDir = '';
  form.codeWorkspace = '';
  form.terminalDir = '';
  form.defaultEditor = 'vscode';
  form.defaultEntryDoc = '';
}

function startCreate(): void {
  if (store.isSettingsInvalid) {
    return;
  }
  resetForm();
  formOpen.value = true;
}

function startEdit(ws: WorkspaceConfig): void {
  if (store.isSettingsInvalid) {
    return;
  }
  editingOriginalKey.value = ws.key;
  openDirTouched.value = ws.openDir !== ws.root;
  formError.value = null;
  form.key = ws.key;
  form.name = ws.name;
  form.root = ws.root;
  form.openDir = ws.openDir;
  form.codeWorkspace = ws.codeWorkspace ?? '';
  form.terminalDir = ws.terminalDir ?? '';
  form.defaultEditor = ws.defaultEditor || 'vscode';
  form.defaultEntryDoc = ws.defaultEntryDoc ?? '';
  formOpen.value = true;
}

function cancelEdit(): void {
  formOpen.value = false;
  resetForm();
}

function handleRootUpdate(value: string): void {
  form.root = value;
  if (!openDirTouched.value) {
    form.openDir = value;
  }
}

function handleOpenDirUpdate(value: string): void {
  openDirTouched.value = true;
  form.openDir = value;
}

function toNullableTrimmed(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

async function handleSave(): Promise<void> {
  if (store.isSettingsInvalid) {
    formError.value = SETTINGS_READONLY_ERROR;
    return;
  }

  const key = form.key.trim();
  const name = form.name.trim();
  const root = form.root.trim();
  const openDir = (form.openDir.trim() || root).trim();

  if (!key) {
    formError.value = '工作区 key 不能为空。';
    return;
  }
  if (!name) {
    formError.value = '工作区显示名称不能为空。';
    return;
  }
  if (!root) {
    formError.value = '项目根目录 root 不能为空。';
    return;
  }

  const currentMap = store.snapshot?.settings.workspaces ?? {};
  const isRenaming = editingOriginalKey.value !== key;
  if (isRenaming && Object.prototype.hasOwnProperty.call(currentMap, key)) {
    formError.value = `工作区 key「${key}」已存在，请使用唯一标识。`;
    return;
  }

  formError.value = null;
  const nextWorkspaces: Record<string, WorkspaceConfig> = { ...currentMap };
  if (editingOriginalKey.value && editingOriginalKey.value !== key) {
    delete nextWorkspaces[editingOriginalKey.value];
  }

  nextWorkspaces[key] = {
    key,
    name,
    root,
    openDir,
    codeWorkspace: toNullableTrimmed(form.codeWorkspace),
    terminalDir: toNullableTrimmed(form.terminalDir),
    defaultEditor: form.defaultEditor === 'sublime' ? 'sublime' : 'vscode',
    defaultEntryDoc: toNullableTrimmed(form.defaultEntryDoc),
  };

  const ok = await store.patchSettings({ workspaces: nextWorkspaces });
  if (ok) {
    formOpen.value = false;
    resetForm();
  }
}

async function handleDelete(key: string): Promise<void> {
  if (store.isSettingsInvalid) {
    return;
  }
  const currentMap = store.snapshot?.settings.workspaces ?? {};
  if (!Object.prototype.hasOwnProperty.call(currentMap, key)) {
    return;
  }
  const nextWorkspaces: Record<string, WorkspaceConfig> = { ...currentMap };
  delete nextWorkspaces[key];
  const ok = await store.patchSettings({ workspaces: nextWorkspaces });
  if (ok && editingOriginalKey.value === key) {
    cancelEdit();
  }
}
</script>

<template>
  <div class="qn-workspaces-panel" data-testid="workspaces-panel">
    <QnBanner
      v-if="store.isSettingsInvalid"
      tone="warning"
      title="配置已只读保护"
      data-testid="workspaces-readonly-banner"
    >
      {{ SETTINGS_READONLY_ERROR }}
    </QnBanner>

    <QnPanelCard
      title="工作区配置"
      subtitle="为 frontmatter 的 project 字段绑定项目目录、外部编辑器与 Quick Bar 默认入口"
    >
      <template #header>
        <QnButton
          variant="primary"
          size="sm"
          :disabled="store.isSettingsInvalid || store.saving"
          data-testid="workspace-new-btn"
          @click="startCreate"
        >
          新增工作区
        </QnButton>
      </template>

      <div v-if="workspaceList.length === 0" class="qn-workspaces-panel__empty">
        暂无工作区配置。点击右上角「新增工作区」创建第一个项目映射。
      </div>

      <div v-else class="qn-workspaces-panel__list">
        <div
          v-for="ws in workspaceList"
          :key="ws.key"
          class="qn-workspaces-panel__item"
          :data-testid="`workspace-row-${ws.key}`"
        >
          <div class="qn-workspaces-panel__item-main">
            <div class="qn-workspaces-panel__item-title">
              <span class="qn-workspaces-panel__item-name">{{ ws.name }}</span>
              <QnBadge tone="neutral">{{ ws.key }}</QnBadge>
              <QnBadge tone="accent">{{ ws.defaultEditor }}</QnBadge>
            </div>
            <div class="qn-workspaces-panel__item-meta">
              <code>root: {{ ws.root }}</code>
              <code v-if="ws.openDir !== ws.root">openDir: {{ ws.openDir }}</code>
              <code v-if="ws.codeWorkspace">codeWorkspace: {{ ws.codeWorkspace }}</code>
              <code v-if="ws.terminalDir">terminalDir: {{ ws.terminalDir }}</code>
              <code v-if="ws.defaultEntryDoc">defaultEntryDoc: {{ ws.defaultEntryDoc }}</code>
            </div>
          </div>
          <div class="qn-workspaces-panel__item-actions">
            <QnButton
              variant="secondary"
              size="sm"
              :disabled="store.isSettingsInvalid || store.saving"
              :data-testid="`workspace-edit-${ws.key}`"
              @click="startEdit(ws)"
            >
              编辑
            </QnButton>
            <QnButton
              variant="danger"
              size="sm"
              :disabled="store.isSettingsInvalid || store.saving"
              :data-testid="`workspace-delete-${ws.key}`"
              @click="handleDelete(ws.key)"
            >
              删除
            </QnButton>
          </div>
        </div>
      </div>
    </QnPanelCard>

    <QnPanelCard
      v-if="formOpen"
      :title="editingOriginalKey ? `编辑工作区：${editingOriginalKey}` : '新建工作区'"
      subtitle="所有路径建议使用绝对路径（defaultEntryDoc 为库内相对路径）"
      data-testid="workspace-form"
    >
      <div class="qn-workspaces-panel__form">
        <div class="qn-workspaces-panel__grid">
          <QnInput
            v-model="form.key"
            label="工作区 Key（frontmatter project 引用）"
            placeholder="例如：quicknote"
            :disabled="store.isSettingsInvalid"
            monospace
            data-testid="workspace-input-key"
          />
          <QnInput
            v-model="form.name"
            label="显示名称"
            placeholder="例如：QuickNote 主仓库"
            :disabled="store.isSettingsInvalid"
            data-testid="workspace-input-name"
          />
        </div>

        <QnInput
          :model-value="form.root"
          label="项目根目录 (root)"
          placeholder="/Users/name/Sites/Project"
          :disabled="store.isSettingsInvalid"
          monospace
          data-testid="workspace-input-root"
          @update:model-value="handleRootUpdate"
        />

        <QnInput
          :model-value="form.openDir"
          label="编辑器打开目录 (openDir)"
          placeholder="默认与 root 一致"
          :disabled="store.isSettingsInvalid"
          monospace
          data-testid="workspace-input-openDir"
          @update:model-value="handleOpenDirUpdate"
        />

        <div class="qn-workspaces-panel__grid">
          <QnInput
            v-model="form.codeWorkspace"
            label=".code-workspace 路径（可选）"
            placeholder="/Users/name/Sites/Project/app.code-workspace"
            :disabled="store.isSettingsInvalid"
            monospace
            data-testid="workspace-input-codeWorkspace"
          />
          <QnInput
            v-model="form.terminalDir"
            label="终端初始目录覆盖（可选）"
            placeholder="留空则回退 openDir / root"
            :disabled="store.isSettingsInvalid"
            monospace
            data-testid="workspace-input-terminalDir"
          />
        </div>

        <div class="qn-workspaces-panel__grid">
          <label class="qn-workspaces-panel__field">
            <span class="qn-workspaces-panel__label">默认外部编辑器 (defaultEditor)</span>
            <select
              v-model="form.defaultEditor"
              class="qn-workspaces-panel__select"
              :disabled="store.isSettingsInvalid"
              data-testid="workspace-select-defaultEditor"
            >
              <option value="vscode">vscode</option>
              <option value="sublime">sublime</option>
            </select>
          </label>

          <QnInput
            v-model="form.defaultEntryDoc"
            label="Quick Bar 默认入口文档（库内相对路径，可选）"
            placeholder="例如：projects/quicknote.md"
            :disabled="store.isSettingsInvalid"
            monospace
            data-testid="workspace-input-defaultEntryDoc"
          />
        </div>

        <p
          v-if="formError"
          class="qn-workspaces-panel__error"
          role="alert"
          data-testid="workspace-form-error"
        >
          {{ formError }}
        </p>

        <div class="qn-workspaces-panel__form-actions">
          <QnButton
            variant="ghost"
            size="sm"
            data-testid="workspace-cancel-btn"
            @click="cancelEdit"
          >
            取消
          </QnButton>
          <QnButton
            variant="primary"
            size="sm"
            :loading="store.saving"
            :disabled="store.isSettingsInvalid"
            data-testid="workspace-save-btn"
            @click="handleSave"
          >
            保存工作区
          </QnButton>
        </div>
      </div>
    </QnPanelCard>
  </div>
</template>

<style scoped>
.qn-workspaces-panel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-workspaces-panel__empty {
  font-size: var(--qn-text-sm);
  color: var(--qn-text-muted);
}

.qn-workspaces-panel__list {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-workspaces-panel__item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--qn-space-4);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-workspaces-panel__item-main {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  min-width: 0;
}

.qn-workspaces-panel__item-title {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}

.qn-workspaces-panel__item-name {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-workspaces-panel__item-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-workspaces-panel__item-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}

.qn-workspaces-panel__form {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-workspaces-panel__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--qn-space-3);
}

.qn-workspaces-panel__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-workspaces-panel__label {
  font-size: var(--qn-text-xs);
  font-weight: 500;
  color: var(--qn-text-secondary);
}

.qn-workspaces-panel__select {
  height: 32px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-workspaces-panel__error {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}

.qn-workspaces-panel__form-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--qn-space-2);
}
</style>
