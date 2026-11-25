<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import type {
  DirNode,
  DirectoryWorkspaceConfig,
  DirectoryWorkspaceConfigEntry,
} from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnInput, QnPanelCard } from '@/components';
import {
  listDirectoryWorkspaceConfigs,
  notesGetTreeAndTags,
  setDirectoryWorkspaceConfig,
} from '@/lib/ipc/commands';
import { extractSettingsErrorMessage } from './api';
import { SETTINGS_READONLY_ERROR, useSettingsStore } from './store';

const store = useSettingsStore();

const entries = ref<DirectoryWorkspaceConfigEntry[]>([]);
const vaultDirs = ref<string[]>([]);
const loadingList = ref(false);
const savingBinding = ref(false);
const loadError = ref<string | null>(null);
const formError = ref<string | null>(null);

const formOpen = ref(false);
const editingOriginalDir = ref<string | null>(null);

const form = reactive({
  vaultRelDir: '',
  vscodeWorkspacePath: '',
  projectRoot: '',
  defaultTerminalCwd: '',
});

const sortedEntries = computed<DirectoryWorkspaceConfigEntry[]>(() =>
  [...entries.value].sort((a, b) => a.vaultRelDir.localeCompare(b.vaultRelDir)),
);

function flattenDirNodes(nodes: DirNode[], out: string[] = []): string[] {
  for (const node of nodes) {
    if (node.relPath) {
      out.push(node.relPath);
    }
    if (node.children.length > 0) {
      flattenDirNodes(node.children, out);
    }
  }
  return out;
}

function getEntryRowId(vaultRelDir: string): string {
  return vaultRelDir.trim().length > 0 ? vaultRelDir : 'root';
}

function formatDirLabel(vaultRelDir: string): string {
  return vaultRelDir.trim().length > 0 ? vaultRelDir : '（笔记库根目录）';
}

function readVscodeWorkspacePath(config: DirectoryWorkspaceConfig | null): string | null {
  return config?.vscodeWorkspacePath ?? config?.vscode_workspace_path ?? null;
}

function readProjectRoot(config: DirectoryWorkspaceConfig | null): string | null {
  return config?.projectRoot ?? config?.project_root ?? null;
}

function readDefaultTerminalCwd(config: DirectoryWorkspaceConfig | null): string | null {
  return config?.defaultTerminalCwd ?? config?.default_terminal_cwd ?? null;
}

function toNullableTrimmed(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function normalizeVaultRelDir(raw: string): string | null {
  const replaced = raw.trim().replace(/\\/g, '/');
  if (replaced.startsWith('/')) {
    return null;
  }
  const parts: string[] = [];
  for (const seg of replaced.split('/')) {
    if (seg === '' || seg === '.') {
      continue;
    }
    if (seg === '..') {
      return null;
    }
    parts.push(seg);
  }
  return parts.join('/');
}

async function loadDirectoryWorkspaces(): Promise<void> {
  loadingList.value = true;
  loadError.value = null;
  try {
    const list = await listDirectoryWorkspaceConfigs();
    entries.value = list;
  } catch (error) {
    loadError.value = extractSettingsErrorMessage(error);
  } finally {
    loadingList.value = false;
  }

  try {
    const tree = await notesGetTreeAndTags();
    vaultDirs.value = Array.from(new Set(flattenDirNodes(tree.dirs))).sort((a, b) =>
      a.localeCompare(b),
    );
  } catch {
    // 目录树可选辅助，失败时不阻断工作区配置管理
  }
}

function resetForm(): void {
  editingOriginalDir.value = null;
  formError.value = null;
  form.vaultRelDir = '';
  form.vscodeWorkspacePath = '';
  form.projectRoot = '';
  form.defaultTerminalCwd = '';
}

function startCreate(): void {
  resetForm();
  formOpen.value = true;
}

function startEdit(entry: DirectoryWorkspaceConfigEntry): void {
  editingOriginalDir.value = entry.vaultRelDir;
  formError.value = null;
  form.vaultRelDir = entry.vaultRelDir;
  form.vscodeWorkspacePath = readVscodeWorkspacePath(entry.config) ?? '';
  form.projectRoot = readProjectRoot(entry.config) ?? '';
  form.defaultTerminalCwd = readDefaultTerminalCwd(entry.config) ?? '';
  formOpen.value = true;
}

function cancelEdit(): void {
  formOpen.value = false;
  resetForm();
}

function onSelectVaultDir(event: Event): void {
  const target = event.target as HTMLSelectElement | null;
  if (!target) {
    return;
  }
  form.vaultRelDir = target.value;
}

async function handleSave(): Promise<void> {
  const normalizedDir = normalizeVaultRelDir(form.vaultRelDir);
  if (normalizedDir === null) {
    formError.value = '目录路径必须为笔记库内相对路径（不可包含前导 / 或 ..）。';
    return;
  }

  const vscodeWorkspacePath = toNullableTrimmed(form.vscodeWorkspacePath);
  const projectRoot = toNullableTrimmed(form.projectRoot);
  const defaultTerminalCwd = toNullableTrimmed(form.defaultTerminalCwd);

  if (!vscodeWorkspacePath && !projectRoot && !defaultTerminalCwd) {
    formError.value =
      '请至少填写 VS Code 工作区路径、代码根目录或默认终端 cwd 之一；如需清除绑定请点击「解除绑定」。';
    return;
  }

  savingBinding.value = true;
  formError.value = null;
  try {
    const payload: DirectoryWorkspaceConfig = {
      vscodeWorkspacePath,
      projectRoot,
      defaultTerminalCwd,
    };
    await setDirectoryWorkspaceConfig({
      vaultRelDir: normalizedDir,
      config: payload,
    });
    await loadDirectoryWorkspaces();
    formOpen.value = false;
    resetForm();
  } catch (error) {
    formError.value = extractSettingsErrorMessage(error);
  } finally {
    savingBinding.value = false;
  }
}

async function handleUnbind(vaultRelDir: string): Promise<void> {
  savingBinding.value = true;
  loadError.value = null;
  try {
    await setDirectoryWorkspaceConfig({
      vaultRelDir,
      config: null,
    });
    await loadDirectoryWorkspaces();
    if (editingOriginalDir.value === vaultRelDir) {
      cancelEdit();
    }
  } catch (error) {
    loadError.value = extractSettingsErrorMessage(error);
  } finally {
    savingBinding.value = false;
  }
}

onMounted(() => {
  void loadDirectoryWorkspaces();
});
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
      title="笔记库目录工作区绑定 (.quicknote.yaml)"
      subtitle="为笔记库目录绑定磁盘 .quicknote.yaml，按「文档 Frontmatter > 最近祖先目录 > 全局默认」级联解析外部动作与终端目录"
    >
      <template #header>
        <div class="qn-workspaces-panel__header-actions">
          <QnButton
            variant="ghost"
            size="sm"
            :disabled="loadingList || savingBinding"
            data-testid="workspace-refresh-btn"
            @click="loadDirectoryWorkspaces"
          >
            刷新
          </QnButton>
          <QnButton
            variant="primary"
            size="sm"
            :disabled="savingBinding"
            data-testid="workspace-new-btn"
            @click="startCreate"
          >
            新增目录绑定
          </QnButton>
        </div>
      </template>

      <p
        v-if="loadError"
        class="qn-workspaces-panel__error"
        role="alert"
        data-testid="workspace-load-error"
      >
        {{ loadError }}
      </p>

      <div v-if="loadingList" class="qn-workspaces-panel__empty">
        正在读取笔记库目录工作区绑定…
      </div>

      <div
        v-else-if="sortedEntries.length === 0"
        class="qn-workspaces-panel__empty"
        data-testid="workspace-empty-state"
      >
        暂无目录级 .quicknote.yaml 绑定。点击右上角「新增目录绑定」为笔记库目录关联代码仓库与终端目录。
      </div>

      <div v-else class="qn-workspaces-panel__list">
        <div
          v-for="entry in sortedEntries"
          :key="entry.vaultRelDir || '__root__'"
          class="qn-workspaces-panel__item"
          :data-testid="`workspace-row-${getEntryRowId(entry.vaultRelDir)}`"
        >
          <div class="qn-workspaces-panel__item-main">
            <div class="qn-workspaces-panel__item-title">
              <span class="qn-workspaces-panel__item-name">
                {{ formatDirLabel(entry.vaultRelDir) }}
              </span>
              <QnBadge tone="neutral">.quicknote.yaml</QnBadge>
              <QnBadge v-if="entry.parseError" tone="warning">解析异常</QnBadge>
            </div>

            <p v-if="entry.parseError" class="qn-workspaces-panel__error" role="alert">
              {{ entry.parseError }}
            </p>

            <div v-else class="qn-workspaces-panel__item-meta">
              <code v-if="readProjectRoot(entry.config)">
                代码根目录: {{ readProjectRoot(entry.config) }}
              </code>
              <code v-if="readVscodeWorkspacePath(entry.config)">
                VS Code 工作区: {{ readVscodeWorkspacePath(entry.config) }}
              </code>
              <code v-if="readDefaultTerminalCwd(entry.config)">
                默认终端 cwd: {{ readDefaultTerminalCwd(entry.config) }}
              </code>
            </div>
          </div>

          <div class="qn-workspaces-panel__item-actions">
            <QnButton
              variant="secondary"
              size="sm"
              :disabled="savingBinding"
              :data-testid="`workspace-edit-${getEntryRowId(entry.vaultRelDir)}`"
              @click="startEdit(entry)"
            >
              编辑
            </QnButton>
            <QnButton
              variant="danger"
              size="sm"
              :disabled="savingBinding"
              :data-testid="`workspace-unbind-${getEntryRowId(entry.vaultRelDir)}`"
              @click="handleUnbind(entry.vaultRelDir)"
            >
              解除绑定
            </QnButton>
          </div>
        </div>
      </div>
    </QnPanelCard>

    <QnPanelCard
      v-if="formOpen"
      :title="
        editingOriginalDir !== null
          ? `编辑目录绑定：${formatDirLabel(editingOriginalDir)}`
          : '新增目录工作区绑定'
      "
      subtitle="保存后将在目标目录原子写入 .quicknote.yaml 并同步更新索引缓存"
      data-testid="workspace-form"
    >
      <div class="qn-workspaces-panel__form">
        <div v-if="editingOriginalDir === null && vaultDirs.length > 0" class="qn-workspaces-panel__field">
          <label class="qn-workspaces-panel__field">
            <span class="qn-workspaces-panel__label">从笔记库已有目录选择</span>
            <select
              :value="form.vaultRelDir"
              class="qn-workspaces-panel__select"
              :disabled="savingBinding"
              data-testid="workspace-dir-select"
              @change="onSelectVaultDir"
            >
              <option value="">（笔记库根目录）</option>
              <option v-for="dir in vaultDirs" :key="dir" :value="dir">
                {{ dir }}
              </option>
            </select>
          </label>
        </div>

        <QnInput
          v-model="form.vaultRelDir"
          label="笔记库目录相对路径（留空表示根目录）"
          placeholder="例如：projects/quicknote"
          :disabled="editingOriginalDir !== null || savingBinding"
          monospace
          data-testid="workspace-input-dir"
        />

        <QnInput
          v-model="form.projectRoot"
          label="代码根目录 (projectRoot)"
          placeholder="/Users/name/Sites/Project"
          :disabled="savingBinding"
          monospace
          data-testid="workspace-input-projectRoot"
        />

        <div class="qn-workspaces-panel__grid">
          <QnInput
            v-model="form.vscodeWorkspacePath"
            label="VS Code 工作区路径 (vscodeWorkspacePath)"
            placeholder="/Users/name/Sites/Project/app.code-workspace"
            :disabled="savingBinding"
            monospace
            data-testid="workspace-input-vscodeWorkspacePath"
          />
          <QnInput
            v-model="form.defaultTerminalCwd"
            label="默认终端 cwd (defaultTerminalCwd)"
            placeholder="留空则回退代码根目录"
            :disabled="savingBinding"
            monospace
            data-testid="workspace-input-defaultTerminalCwd"
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
            :disabled="savingBinding"
            data-testid="workspace-cancel-btn"
            @click="cancelEdit"
          >
            取消
          </QnButton>
          <QnButton
            variant="primary"
            size="sm"
            :loading="savingBinding"
            data-testid="workspace-save-btn"
            @click="handleSave"
          >
            保存 .quicknote.yaml
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

.qn-workspaces-panel__header-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
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
  gap: var(--qn-space-1);
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
