<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type {
  ExternalAppConfig,
  ExternalBookmark,
  ExternalBookmarkKind,
} from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnInput, QnPanelCard } from '@/components';
import { removeExternalBookmark, upsertExternalBookmark } from '@/lib/ipc/commands';
import { extractSettingsErrorMessage } from './api';
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
const embeddedTerminalForm = reactive({
  shell: '/bin/zsh',
  idleRecycleMinutes: '30',
  maxConcurrentSessions: '3',
});

const bookmarkForm = reactive<{
  path: string;
  kind: ExternalBookmarkKind;
  alias: string;
  pinned: boolean;
}>({
  path: '',
  kind: 'directory',
  alias: '',
  pinned: false,
});

const scanHiddenFiles = ref(true);
const bookmarkBusy = ref(false);
const terminalError = ref<string | null>(null);
const embeddedTerminalError = ref<string | null>(null);
const editorsError = ref<string | null>(null);
const bookmarksError = ref<string | null>(null);

const externalBookmarks = computed<ExternalBookmark[]>(
  () => store.snapshot?.settings.externalBookmarks ?? [],
);

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
    embeddedTerminalForm.shell = settings.panels.terminal.shell;
    embeddedTerminalForm.idleRecycleMinutes = String(settings.panels.terminal.idleRecycleMinutes);
    embeddedTerminalForm.maxConcurrentSessions = String(
      settings.panels.terminal.maxConcurrentSessions,
    );
    scanHiddenFiles.value = settings.scanHiddenFiles ?? true;
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

async function saveEmbeddedTerminalConfig(): Promise<void> {
  if (store.isSettingsInvalid) {
    embeddedTerminalError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  const shell = embeddedTerminalForm.shell.trim();
  if (!shell) {
    embeddedTerminalError.value = '登录 Shell 路径不能为空。';
    return;
  }
  const idleRaw = Number(embeddedTerminalForm.idleRecycleMinutes);
  const idleRecycleMinutes = Number.isFinite(idleRaw) ? Math.max(0, Math.round(idleRaw)) : 30;
  const maxRaw = Number(embeddedTerminalForm.maxConcurrentSessions);
  const maxConcurrentSessions = Number.isFinite(maxRaw)
    ? Math.min(6, Math.max(1, Math.round(maxRaw)))
    : 3;

  embeddedTerminalForm.idleRecycleMinutes = String(idleRecycleMinutes);
  embeddedTerminalForm.maxConcurrentSessions = String(maxConcurrentSessions);
  embeddedTerminalError.value = null;

  await store.patchSettings({
    panels: {
      terminal: {
        shell,
        idleRecycleMinutes,
        maxConcurrentSessions,
      },
    },
  });
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

function syncBookmarksSnapshot(nextList: ExternalBookmark[]): void {
  if (!store.snapshot) {
    return;
  }
  store.snapshot = {
    ...store.snapshot,
    settings: {
      ...store.snapshot.settings,
      externalBookmarks: nextList,
    },
  };
}

async function onToggleScanHiddenFiles(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement | null;
  if (!target) {
    return;
  }
  if (store.isSettingsInvalid) {
    bookmarksError.value = SETTINGS_READONLY_ERROR;
    target.checked = scanHiddenFiles.value;
    return;
  }
  bookmarksError.value = null;
  scanHiddenFiles.value = target.checked;
  await store.patchSettings({ scanHiddenFiles: scanHiddenFiles.value });
}

async function addBookmark(): Promise<void> {
  if (store.isSettingsInvalid) {
    bookmarksError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  const trimmedPath = bookmarkForm.path.trim();
  if (!trimmedPath) {
    bookmarksError.value = '书签路径不能为空。';
    return;
  }
  const trimmedAlias = bookmarkForm.alias.trim();

  bookmarkBusy.value = true;
  bookmarksError.value = null;
  try {
    const updated = await upsertExternalBookmark({
      path: trimmedPath,
      kind: bookmarkForm.kind,
      alias: trimmedAlias.length > 0 ? trimmedAlias : null,
      pinned: bookmarkForm.pinned,
      lastOpenedAt: null,
    });
    syncBookmarksSnapshot(updated);
    bookmarkForm.path = '';
    bookmarkForm.alias = '';
    bookmarkForm.pinned = false;
  } catch (error) {
    bookmarksError.value = extractSettingsErrorMessage(error);
  } finally {
    bookmarkBusy.value = false;
  }
}

async function toggleBookmarkPinned(item: ExternalBookmark): Promise<void> {
  if (store.isSettingsInvalid) {
    bookmarksError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  bookmarkBusy.value = true;
  bookmarksError.value = null;
  try {
    const updated = await upsertExternalBookmark({
      ...item,
      pinned: !item.pinned,
    });
    syncBookmarksSnapshot(updated);
  } catch (error) {
    bookmarksError.value = extractSettingsErrorMessage(error);
  } finally {
    bookmarkBusy.value = false;
  }
}

async function deleteBookmark(targetPath: string): Promise<void> {
  if (store.isSettingsInvalid) {
    bookmarksError.value = SETTINGS_READONLY_ERROR;
    return;
  }
  bookmarkBusy.value = true;
  bookmarksError.value = null;
  try {
    const updated = await removeExternalBookmark(targetPath);
    syncBookmarksSnapshot(updated);
  } catch (error) {
    bookmarksError.value = extractSettingsErrorMessage(error);
  } finally {
    bookmarkBusy.value = false;
  }
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

    <QnPanelCard
      title="内嵌终端面板配置"
      subtitle="配置右侧内置终端的登录 Shell、闲置回收与并发会话上限"
      data-testid="settings-terminal-panel-card"
    >
      <div class="qn-external-panel__stack">
        <QnInput
          v-model="embeddedTerminalForm.shell"
          label="登录 Shell 路径"
          placeholder="/bin/zsh"
          monospace
          :disabled="store.isSettingsInvalid"
          data-testid="embedded-terminal-shell-input"
          @change="saveEmbeddedTerminalConfig"
        />

        <div class="qn-external-panel__grid">
          <QnInput
            v-model="embeddedTerminalForm.idleRecycleMinutes"
            type="number"
            min="0"
            label="隐藏闲置自动回收（分钟，0 表示禁用自动回收）"
            :disabled="store.isSettingsInvalid"
            data-testid="embedded-terminal-idle-input"
            @change="saveEmbeddedTerminalConfig"
          />
          <QnInput
            v-model="embeddedTerminalForm.maxConcurrentSessions"
            type="number"
            min="1"
            max="6"
            label="全局最大并发会话数（1–6）"
            :disabled="store.isSettingsInvalid"
            data-testid="embedded-terminal-max-sessions-input"
            @change="saveEmbeddedTerminalConfig"
          />
        </div>

        <p v-if="embeddedTerminalError" class="qn-external-panel__error" role="alert">
          {{ embeddedTerminalError }}
        </p>
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
      title="外部常用目录与文件书签"
      subtitle="管理库外常用目录与文件书签，供 Quick Bar 与快速打开直接浏览和探测"
      data-testid="external-bookmarks-card"
    >
      <div class="qn-external-panel__stack">
        <label class="qn-external-panel__checkbox">
          <input
            :checked="scanHiddenFiles"
            type="checkbox"
            :disabled="store.isSettingsInvalid || store.saving"
            data-testid="external-scan-hidden-checkbox"
            @change="onToggleScanHiddenFiles"
          />
          <span>目录直扫时包含隐藏目录与隐藏文件（如 .config、.ssh、.env）</span>
        </label>

        <div class="qn-external-panel__bookmark-form">
          <div class="qn-external-panel__grid">
            <QnInput
              v-model="bookmarkForm.path"
              label="目录或文件绝对路径"
              placeholder="/Users/name/Sites 或 ~/.zshrc"
              :disabled="store.isSettingsInvalid || bookmarkBusy"
              monospace
              data-testid="external-bookmark-path-input"
            />
            <QnInput
              v-model="bookmarkForm.alias"
              label="显示别名（可选）"
              placeholder="例如：本地项目 / Shell 配置"
              :disabled="store.isSettingsInvalid || bookmarkBusy"
              data-testid="external-bookmark-alias-input"
            />
          </div>

          <div class="qn-external-panel__bookmark-controls">
            <label class="qn-external-panel__field">
              <span class="qn-external-panel__label">书签类型</span>
              <select
                v-model="bookmarkForm.kind"
                class="qn-external-panel__select"
                :disabled="store.isSettingsInvalid || bookmarkBusy"
                data-testid="external-bookmark-kind-select"
              >
                <option value="directory">目录书签</option>
                <option value="file">文件书签</option>
              </select>
            </label>

            <label class="qn-external-panel__checkbox">
              <input
                v-model="bookmarkForm.pinned"
                type="checkbox"
                :disabled="store.isSettingsInvalid || bookmarkBusy"
                data-testid="external-bookmark-pinned-checkbox"
              />
              <span>置顶显示</span>
            </label>

            <QnButton
              variant="secondary"
              size="sm"
              :loading="bookmarkBusy"
              :disabled="store.isSettingsInvalid"
              data-testid="external-bookmark-add-btn"
              @click="addBookmark"
            >
              添加书签
            </QnButton>
          </div>
        </div>

        <p
          v-if="bookmarksError"
          class="qn-external-panel__error"
          role="alert"
          data-testid="external-bookmarks-error"
        >
          {{ bookmarksError }}
        </p>

        <div
          v-if="externalBookmarks.length === 0"
          class="qn-external-panel__empty"
          data-testid="external-bookmarks-empty"
        >
          暂未添加外部目录或文件书签。
        </div>

        <ul v-else class="qn-external-panel__bookmark-list">
          <li
            v-for="(item, idx) in externalBookmarks"
            :key="item.path"
            class="qn-external-panel__bookmark-item"
            :data-testid="`external-bookmark-item-${idx}`"
          >
            <div class="qn-external-panel__bookmark-main">
              <div class="qn-external-panel__bookmark-head">
                <QnBadge :tone="item.kind === 'directory' ? 'accent' : 'neutral'">
                  {{ item.kind === 'directory' ? '目录' : '文件' }}
                </QnBadge>
                <QnBadge v-if="item.pinned" tone="warning">置顶</QnBadge>
                <span v-if="item.alias" class="qn-external-panel__bookmark-alias">
                  {{ item.alias }}
                </span>
              </div>
              <code class="qn-external-panel__bookmark-path">{{ item.path }}</code>
            </div>

            <div class="qn-external-panel__bookmark-actions">
              <QnButton
                variant="ghost"
                size="sm"
                :disabled="store.isSettingsInvalid || bookmarkBusy"
                :data-testid="`external-bookmark-pin-${idx}`"
                @click="toggleBookmarkPinned(item)"
              >
                {{ item.pinned ? '取消置顶' : '置顶' }}
              </QnButton>
              <QnButton
                variant="ghost"
                size="sm"
                :disabled="store.isSettingsInvalid || bookmarkBusy"
                :data-testid="`external-bookmark-remove-${idx}`"
                @click="deleteBookmark(item.path)"
              >
                移除
              </QnButton>
            </div>
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

.qn-external-panel__checkbox {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
}

.qn-external-panel__bookmark-form {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-external-panel__bookmark-controls {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--qn-space-3);
}

.qn-external-panel__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-external-panel__label {
  font-size: var(--qn-text-xs);
  font-weight: 500;
  color: var(--qn-text-secondary);
}

.qn-external-panel__select {
  height: 32px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-external-panel__bookmark-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-external-panel__bookmark-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-external-panel__bookmark-main {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  min-width: 0;
}

.qn-external-panel__bookmark-head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}

.qn-external-panel__bookmark-alias {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-external-panel__bookmark-path {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  word-break: break-all;
}

.qn-external-panel__bookmark-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex-shrink: 0;
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
