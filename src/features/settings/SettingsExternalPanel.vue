<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type { ExternalAppConfig } from '@/bindings/types';
import { QnBanner, QnButton, QnInput, QnPanelCard } from '@/components';
import { SETTINGS_READONLY_ERROR, useSettingsStore } from './store';

interface TerminalPreset {
  id: string;
  name: string;
  bundleId: string;
}

const TERMINAL_PRESETS: TerminalPreset[] = [
  { id: 'terminal', name: 'Terminal.app', bundleId: 'com.apple.Terminal' },
  { id: 'iterm2', name: 'iTerm2', bundleId: 'com.googlecode.iterm2' },
  { id: 'warp', name: 'Warp', bundleId: 'dev.warp.Warp-Stable' },
  { id: 'ghostty', name: 'Ghostty', bundleId: 'com.mitchellh.ghostty' },
];

const DEFAULT_VSCODE: ExternalAppConfig = {
  name: 'Visual Studio Code',
  bundleId: 'com.microsoft.VSCode',
};

const DEFAULT_SUBLIME: ExternalAppConfig = {
  name: 'Sublime Text',
  bundleId: 'com.sublimetext.4',
};

const store = useSettingsStore();

const terminalForm = reactive<ExternalAppConfig>({
  name: 'Terminal.app',
  bundleId: 'com.apple.Terminal',
});

const vscodeForm = reactive<ExternalAppConfig>({ ...DEFAULT_VSCODE });
const sublimeForm = reactive<ExternalAppConfig>({ ...DEFAULT_SUBLIME });

const newSearchRoot = ref('');
const terminalError = ref<string | null>(null);
const editorsError = ref<string | null>(null);
const searchRootsError = ref<string | null>(null);

const searchRoots = computed<string[]>(() => store.snapshot?.settings.searchRoots ?? []);

watch(
  () => store.snapshot?.settings,
  (settings) => {
    if (!settings) {
      return;
    }
    terminalForm.name = settings.terminal.name;
    terminalForm.bundleId = settings.terminal.bundleId;
    vscodeForm.name = settings.editors.vscode.name;
    vscodeForm.bundleId = settings.editors.vscode.bundleId;
    sublimeForm.name = settings.editors.sublime.name;
    sublimeForm.bundleId = settings.editors.sublime.bundleId;
  },
  { immediate: true, deep: true },
);

async function applyTerminalPreset(preset: TerminalPreset): Promise<void> {
  if (store.isSettingsInvalid) {
    terminalError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  terminalError.value = null;
  terminalForm.name = preset.name;
  terminalForm.bundleId = preset.bundleId;
  await store.patchSettings({
    terminal: { name: preset.name, bundleId: preset.bundleId },
  });
}

async function saveTerminalConfig(): Promise<void> {
  if (store.isSettingsInvalid) {
    terminalError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  const name = terminalForm.name.trim();
  const bundleId = terminalForm.bundleId.trim();
  if (!name || !bundleId) {
    terminalError.value = '终端名称与 Bundle ID 均不能为空。';
    return;
  }
  terminalError.value = null;
  await store.patchSettings({ terminal: { name, bundleId } });
}

function resetVscodeDefault(): void {
  if (store.isSettingsInvalid) {
    return;
  }
  vscodeForm.name = DEFAULT_VSCODE.name;
  vscodeForm.bundleId = DEFAULT_VSCODE.bundleId;
}

function resetSublimeDefault(): void {
  if (store.isSettingsInvalid) {
    return;
  }
  sublimeForm.name = DEFAULT_SUBLIME.name;
  sublimeForm.bundleId = DEFAULT_SUBLIME.bundleId;
}

async function saveEditorsConfig(): Promise<void> {
  if (store.isSettingsInvalid) {
    editorsError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  const vscodeName = vscodeForm.name.trim();
  const vscodeBundleId = vscodeForm.bundleId.trim();
  const sublimeName = sublimeForm.name.trim();
  const sublimeBundleId = sublimeForm.bundleId.trim();

  if (!vscodeName || !vscodeBundleId || !sublimeName || !sublimeBundleId) {
    editorsError.value = '外部编辑器的名称与 Bundle ID 均不能为空。';
    return;
  }

  editorsError.value = null;
  await store.patchSettings({
    editors: {
      vscode: { name: vscodeName, bundleId: vscodeBundleId },
      sublime: { name: sublimeName, bundleId: sublimeBundleId },
    },
  });
}

async function addSearchRoot(): Promise<void> {
  if (store.isSettingsInvalid) {
    searchRootsError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  const trimmed = newSearchRoot.value.trim();
  if (!trimmed) {
    searchRootsError.value = '搜索目录路径不能为空。';
    return;
  }
  if (searchRoots.value.includes(trimmed)) {
    searchRootsError.value = `目录「${trimmed}」已在搜索列表中。`;
    return;
  }
  searchRootsError.value = null;
  const ok = await store.patchSettings({
    searchRoots: [...searchRoots.value, trimmed],
  });
  if (ok) {
    newSearchRoot.value = '';
  }
}

async function removeSearchRoot(target: string): Promise<void> {
  if (store.isSettingsInvalid) {
    searchRootsError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  searchRootsError.value = null;
  await store.patchSettings({
    searchRoots: searchRoots.value.filter((item) => item !== target),
  });
}
</script>

<template>
  <div class="qn-external-panel" data-testid="external-panel">
    <QnBanner
      v-if="store.isSettingsInvalid"
      tone="warning"
      title="配置已只读保护"
      data-testid="external-readonly-banner"
    >
      {{ SETTINGS_READONLY_ERROR }}
    </QnBanner>

    <QnPanelCard
      title="外部终端配置"
      subtitle="选择常用 macOS 终端预设或自定义 Bundle ID，用于「在外部终端中打开」"
    >
      <div class="qn-external-panel__stack">
        <div class="qn-external-panel__presets">
          <QnButton
            v-for="preset in TERMINAL_PRESETS"
            :key="preset.id"
            :variant="terminalForm.bundleId === preset.bundleId ? 'primary' : 'secondary'"
            size="sm"
            :disabled="store.isSettingsInvalid || store.saving"
            :data-testid="`terminal-preset-${preset.id}`"
            @click="applyTerminalPreset(preset)"
          >
            {{ preset.name }}
          </QnButton>
        </div>

        <div class="qn-external-panel__grid">
          <QnInput
            v-model="terminalForm.name"
            label="终端名称"
            placeholder="iTerm2"
            :disabled="store.isSettingsInvalid"
            data-testid="terminal-name-input"
          />
          <QnInput
            v-model="terminalForm.bundleId"
            label="Bundle ID"
            placeholder="com.googlecode.iterm2"
            :disabled="store.isSettingsInvalid"
            monospace
            data-testid="terminal-bundle-id-input"
          />
        </div>

        <p v-if="terminalError" class="qn-external-panel__error" role="alert">
          {{ terminalError }}
        </p>

        <div class="qn-external-panel__actions">
          <QnButton
            variant="primary"
            size="sm"
            :loading="store.saving"
            :disabled="store.isSettingsInvalid"
            data-testid="terminal-save-btn"
            @click="saveTerminalConfig"
          >
            保存终端配置
          </QnButton>
        </div>
      </div>
    </QnPanelCard>

    <QnPanelCard title="外部编辑器配置" subtitle="配置 VS Code 与 Sublime Text 的 macOS Bundle ID">
      <div class="qn-external-panel__stack">
        <div class="qn-external-panel__editor-group">
          <div class="qn-external-panel__editor-head">
            <span class="qn-external-panel__editor-title">VS Code</span>
            <QnButton
              variant="ghost"
              size="sm"
              :disabled="store.isSettingsInvalid"
              data-testid="editor-vscode-reset-btn"
              @click="resetVscodeDefault"
            >
              恢复默认
            </QnButton>
          </div>
          <div class="qn-external-panel__grid">
            <QnInput
              v-model="vscodeForm.name"
              label="应用名称"
              :disabled="store.isSettingsInvalid"
              data-testid="editor-vscode-name-input"
            />
            <QnInput
              v-model="vscodeForm.bundleId"
              label="Bundle ID"
              :disabled="store.isSettingsInvalid"
              monospace
              data-testid="editor-vscode-bundle-id-input"
            />
          </div>
        </div>

        <div class="qn-external-panel__editor-group">
          <div class="qn-external-panel__editor-head">
            <span class="qn-external-panel__editor-title">Sublime Text</span>
            <QnButton
              variant="ghost"
              size="sm"
              :disabled="store.isSettingsInvalid"
              data-testid="editor-sublime-reset-btn"
              @click="resetSublimeDefault"
            >
              恢复默认
            </QnButton>
          </div>
          <div class="qn-external-panel__grid">
            <QnInput
              v-model="sublimeForm.name"
              label="应用名称"
              :disabled="store.isSettingsInvalid"
              data-testid="editor-sublime-name-input"
            />
            <QnInput
              v-model="sublimeForm.bundleId"
              label="Bundle ID"
              :disabled="store.isSettingsInvalid"
              monospace
              data-testid="editor-sublime-bundle-id-input"
            />
          </div>
        </div>

        <p v-if="editorsError" class="qn-external-panel__error" role="alert">
          {{ editorsError }}
        </p>

        <div class="qn-external-panel__actions">
          <QnButton
            variant="primary"
            size="sm"
            :loading="store.saving"
            :disabled="store.isSettingsInvalid"
            data-testid="editors-save-btn"
            @click="saveEditorsConfig"
          >
            保存编辑器配置
          </QnButton>
        </div>
      </div>
    </QnPanelCard>

    <QnPanelCard
      title="Spotlight 搜索根目录 (searchRoots)"
      subtitle="限制 Spotlight 外部文件检索范围（留空表示不额外限定特定根目录）"
    >
      <div class="qn-external-panel__stack">
        <div class="qn-external-panel__add-row">
          <QnInput
            v-model="newSearchRoot"
            label="新增搜索根目录"
            placeholder="/Users/name/Sites"
            :disabled="store.isSettingsInvalid"
            monospace
            data-testid="search-root-input"
          />
          <QnButton
            variant="secondary"
            size="sm"
            :loading="store.saving"
            :disabled="store.isSettingsInvalid"
            data-testid="search-root-add-btn"
            @click="addSearchRoot"
          >
            添加目录
          </QnButton>
        </div>

        <p
          v-if="searchRootsError"
          class="qn-external-panel__error"
          role="alert"
          data-testid="search-roots-error"
        >
          {{ searchRootsError }}
        </p>

        <div v-if="searchRoots.length === 0" class="qn-external-panel__empty">
          暂未配置自定义 Spotlight 搜索根目录。
        </div>

        <ul v-else class="qn-external-panel__root-list">
          <li
            v-for="(root, idx) in searchRoots"
            :key="root"
            class="qn-external-panel__root-item"
            :data-testid="`search-root-item-${idx}`"
          >
            <code class="qn-external-panel__root-path">{{ root }}</code>
            <QnButton
              variant="ghost"
              size="sm"
              :disabled="store.isSettingsInvalid || store.saving"
              :data-testid="`search-root-remove-${idx}`"
              @click="removeSearchRoot(root)"
            >
              移除
            </QnButton>
          </li>
        </ul>
      </div>
    </QnPanelCard>
  </div>
</template>

<style scoped>
.qn-external-panel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-external-panel__stack {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-external-panel__presets {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}

.qn-external-panel__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--qn-space-3);
}

.qn-external-panel__editor-group {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
}

.qn-external-panel__editor-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.qn-external-panel__editor-title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-external-panel__add-row {
  display: flex;
  align-items: flex-end;
  gap: var(--qn-space-2);
}

.qn-external-panel__add-row > :first-child {
  flex: 1;
}

.qn-external-panel__root-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-external-panel__root-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
}

.qn-external-panel__root-path {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
  word-break: break-all;
}

.qn-external-panel__empty {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-external-panel__error {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}

.qn-external-panel__actions {
  display: flex;
  justify-content: flex-end;
}
</style>
