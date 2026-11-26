<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { DirectoryWorkspaceConfig, DirectoryWorkspaceConfigEntry } from '@/bindings/types';
import { QnBanner, QnButton, QnInput, QnModalShell } from '@/components';
import { extractNotesErrorMessage, saveDirectoryWorkspaceConfig } from './api';
import { extractDirWorkspaceFields, hasBoundDirWorkspaceConfig } from './useDirectoryWorkspaces';

const props = defineProps<{
  open: boolean;
  vaultRelDir: string;
  dirName: string;
  existingEntry: DirectoryWorkspaceConfigEntry | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'saved'): void;
}>();

const vscodePath = ref<string>('');
const projectRoot = ref<string>('');
const terminalCwd = ref<string>('');
const saving = ref<boolean>(false);
const errorMessage = ref<string | null>(null);
const parseError = ref<string | null>(null);

const hasExistingConfig = computed<boolean>(() =>
  Boolean(props.vaultRelDir && hasBoundDirWorkspaceConfig(props.existingEntry ?? undefined)),
);

watch(
  () => [props.open, props.vaultRelDir, props.existingEntry] as const,
  ([isOpen]) => {
    if (!isOpen) {
      return;
    }
    const fields = extractDirWorkspaceFields(props.existingEntry?.config);
    vscodePath.value = fields.vscodeWorkspacePath;
    projectRoot.value = fields.projectRoot;
    terminalCwd.value = fields.defaultTerminalCwd;
    parseError.value = props.existingEntry?.parseError ?? null;
    errorMessage.value = null;
  },
  { immediate: true },
);

function handleClose(): void {
  if (saving.value) {
    return;
  }
  emit('close');
}

async function submitConfig(): Promise<void> {
  if (saving.value || !props.vaultRelDir) {
    return;
  }
  const trimmedVscode = vscodePath.value.trim();
  const trimmedRoot = projectRoot.value.trim();
  const trimmedCwd = terminalCwd.value.trim();
  const payload: DirectoryWorkspaceConfig | null =
    trimmedVscode || trimmedRoot || trimmedCwd
      ? {
          vscodeWorkspacePath: trimmedVscode || null,
          projectRoot: trimmedRoot || null,
          defaultTerminalCwd: trimmedCwd || null,
        }
      : null;

  saving.value = true;
  errorMessage.value = null;
  try {
    await saveDirectoryWorkspaceConfig(props.vaultRelDir, payload);
    emit('saved');
    emit('close');
  } catch (error) {
    errorMessage.value = extractNotesErrorMessage(error);
  } finally {
    saving.value = false;
  }
}

async function clearConfig(): Promise<void> {
  if (saving.value || !props.vaultRelDir) {
    return;
  }
  saving.value = true;
  errorMessage.value = null;
  try {
    await saveDirectoryWorkspaceConfig(props.vaultRelDir, null);
    emit('saved');
    emit('close');
  } catch (error) {
    errorMessage.value = extractNotesErrorMessage(error);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <QnModalShell
    :open="props.open"
    :title="`绑定目录工作区配置 · ${props.dirName || props.vaultRelDir}`"
    max-width="md"
    data-testid="dir-workspace-modal"
    @close="handleClose"
  >
    <div class="qn-dir-ws-modal__body">
      <div class="qn-dir-ws-modal__row">
        <span class="qn-dir-ws-modal__label">目标目录</span>
        <code class="qn-dir-ws-modal__path">{{ props.vaultRelDir }}</code>
      </div>
      <QnBanner v-if="parseError" tone="warning" data-testid="dir-workspace-parse-warning">
        当前目录 `.quicknote.yaml` 解析失败（{{ parseError }}），保存新配置或清除后可恢复正常。
      </QnBanner>
      <div class="qn-dir-ws-modal__row">
        <label class="qn-dir-ws-modal__label" for="qn-dir-ws-vscode-input">
          VS Code 工作区文件（可选）
        </label>
        <QnInput
          id="qn-dir-ws-vscode-input"
          v-model="vscodePath"
          placeholder="例如：/Users/you/Projects/app.code-workspace"
          data-testid="dir-workspace-vscode-input"
        />
      </div>
      <div class="qn-dir-ws-modal__row">
        <label class="qn-dir-ws-modal__label" for="qn-dir-ws-root-input">
          项目根目录（可选）
        </label>
        <QnInput
          id="qn-dir-ws-root-input"
          v-model="projectRoot"
          placeholder="例如：/Users/you/Projects/app"
          data-testid="dir-workspace-root-input"
        />
      </div>
      <div class="qn-dir-ws-modal__row">
        <label class="qn-dir-ws-modal__label" for="qn-dir-ws-cwd-input">
          默认终端目录（可选）
        </label>
        <QnInput
          id="qn-dir-ws-cwd-input"
          v-model="terminalCwd"
          placeholder="例如：/Users/you/Projects/app"
          data-testid="dir-workspace-cwd-input"
          @keydown.enter.prevent="submitConfig"
        />
      </div>
      <p class="qn-dir-ws-modal__hint">
        保存后将写入该目录下的 <code>.quicknote.yaml</code>；全部留空或点击清除将移除配置文件。
      </p>
      <QnBanner v-if="errorMessage" tone="danger" data-testid="dir-workspace-error-banner">
        {{ errorMessage }}
      </QnBanner>
    </div>
    <template #footer>
      <QnButton
        v-if="hasExistingConfig"
        size="sm"
        variant="danger"
        :loading="saving"
        data-testid="dir-workspace-clear-btn"
        @click="clearConfig"
      >
        清除配置
      </QnButton>
      <span class="qn-dir-ws-modal__footer-spacer" />
      <QnButton size="sm" variant="ghost" :disabled="saving" @click="handleClose">取消</QnButton>
      <QnButton
        size="sm"
        variant="primary"
        :loading="saving"
        data-testid="dir-workspace-save-btn"
        @click="submitConfig"
      >
        保存配置
      </QnButton>
    </template>
  </QnModalShell>
</template>

<style scoped>
.qn-dir-ws-modal__body {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-dir-ws-modal__row {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-dir-ws-modal__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-dir-ws-modal__path {
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
  background-color: var(--qn-bg-sidebar);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
}

.qn-dir-ws-modal__hint {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-dir-ws-modal__footer-spacer {
  flex: 1;
}
</style>
