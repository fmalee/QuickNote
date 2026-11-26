<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { QnBadge, QnBanner, QnButton } from '@/components';
import { CommandBookPalette, joinLibraryPath, useCommandPalette } from '@/features/commands';
import { ACTION_KEYBINDINGS_STORAGE_KEY, actionRegistry } from '@/lib/actions';
import QuickBarTargetPicker from './QuickBarTargetPicker.vue';
import { useQuickBarStore, type QuickBarOpenGroupId } from './store';

const store = useQuickBarStore();
const editorRef = ref<HTMLTextAreaElement | null>(null);
const openInputRef = ref<HTMLInputElement | null>(null);
const commandFocusTick = ref(0);
const actionDisposers: Array<() => void> = [];

const commandPaletteEngine = useCommandPalette({
  getContext: () => {
    const root = store.libraryInfo?.root ?? null;
    const relPath = store.uiState?.lastActiveDoc?.relPath ?? null;
    return {
      absPath: root && relPath ? joinLibraryPath(root, relPath) : null,
      workspaceKey: store.resolvedTarget?.workspaceKey ?? null,
    };
  },
  onOpenSource: async (absPath, line) => {
    await store.openFileInMain(absPath, false, line);
  },
  onRequestClose: async () => {
    await store.hide('esc');
  },
});

function focusOpenInput(): void {
  if (store.isUnconfigured || store.mode !== 'open') {
    return;
  }
  void nextTick(() => {
    openInputRef.value?.focus();
  });
}

function getOpenBadgeTone(
  group: QuickBarOpenGroupId,
): 'neutral' | 'accent' | 'warning' | 'success' {
  switch (group) {
    case 'path_direct':
      return 'accent';
    case 'file_bookmark':
      return 'success';
    case 'dir_bookmark':
      return 'warning';
    case 'note':
      return 'neutral';
  }
}

function getOpenItemFlatIndex(groupIndex: number, itemIndex: number): number {
  let offset = 0;
  for (let i = 0; i < groupIndex; i += 1) {
    offset += store.openGroups[i]?.items.length ?? 0;
  }
  return offset + itemIndex;
}

function handleOpenInput(event: Event): void {
  const target = event.target as HTMLInputElement | null;
  store.setOpenQuery(target?.value ?? '');
}

function handleOpenKeydown(event: KeyboardEvent): void {
  if (event.isComposing || event.keyCode === 229) {
    return;
  }

  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    void store.handleEscape();
    return;
  }

  if (event.key === 'ArrowDown') {
    event.preventDefault();
    event.stopPropagation();
    store.moveOpenSelection(1);
    return;
  }

  if (event.key === 'ArrowUp') {
    event.preventDefault();
    event.stopPropagation();
    store.moveOpenSelection(-1);
    return;
  }

  if (event.key === 'Backspace') {
    if (store.openQuery.length === 0 && store.activeOpenDirectoryScope !== null) {
      event.preventDefault();
      event.stopPropagation();
      void store.exitOpenDirectory();
    }
    return;
  }

  if (event.key === 'Enter') {
    event.preventDefault();
    event.stopPropagation();
    void store.confirmOpenSelection();
  }
}

function focusEditor(): void {
  if (
    store.isUnconfigured ||
    store.mode !== 'capture' ||
    store.targetPickerOpen ||
    store.saveAsModalOpen
  ) {
    return;
  }
  void nextTick(() => {
    editorRef.value?.focus();
  });
}

function handleWindowStorage(event: StorageEvent): void {
  if (event.key === ACTION_KEYBINDINGS_STORAGE_KEY || event.key === null) {
    actionRegistry.syncFromStorage();
  }
}

function bindQuickBarActions(): void {
  const canAct = () => !store.isUnconfigured;
  const canActInCapture = () => canAct() && store.mode === 'capture';
  actionDisposers.push(
    actionRegistry.bindHandler(
      'quickbar.mode.capture',
      () => {
        store.mode = 'capture';
      },
      { when: canAct },
    ),
    actionRegistry.bindHandler(
      'quickbar.mode.open',
      () => {
        store.mode = 'open';
      },
      { when: canAct },
    ),
    actionRegistry.bindHandler(
      'quickbar.mode.command',
      () => {
        store.mode = 'command';
      },
      { when: canAct },
    ),
    actionRegistry.bindHandler(
      'quickbar.selectTarget',
      () => {
        store.toggleTargetPicker();
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler(
      'quickbar.captureMode.list',
      () => {
        store.setCaptureMode('list');
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler(
      'quickbar.captureMode.todo',
      () => {
        store.setCaptureMode('todo');
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler(
      'quickbar.captureMode.prose',
      () => {
        store.setCaptureMode('prose');
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler(
      'quickbar.submitAppend',
      () => {
        void store.submitAppend(false);
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler(
      'quickbar.submitOpenMain',
      () => {
        void store.submitAppend(true);
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler(
      'quickbar.saveAsNote',
      () => {
        store.openSaveAsNoteDialog();
      },
      { when: canActInCapture },
    ),
    actionRegistry.bindHandler('quickbar.togglePin', () => {
      void store.togglePin();
    }),
  );
}

onMounted(() => {
  actionRegistry.syncFromStorage();
  bindQuickBarActions();
  if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
    window.addEventListener('storage', handleWindowStorage);
  }
  focusEditor();
});

onUnmounted(() => {
  if (typeof window !== 'undefined' && typeof window.removeEventListener === 'function') {
    window.removeEventListener('storage', handleWindowStorage);
  }
  while (actionDisposers.length > 0) {
    const dispose = actionDisposers.pop();
    dispose?.();
  }
  commandPaletteEngine.dispose();
  store.dispose();
});

watch(
  () =>
    [
      store.focusTick,
      store.isUnconfigured,
      store.mode,
      store.targetPickerOpen,
      store.saveAsModalOpen,
    ] as const,
  ([, unconfigured, currentMode, pickerOpen, saveAsOpen]) => {
    if (unconfigured) {
      return;
    }
    if (currentMode === 'open') {
      void store.refreshOpenMode(true);
      focusOpenInput();
      return;
    }
    if (currentMode === 'command') {
      void commandPaletteEngine.refresh(true);
      commandFocusTick.value += 1;
      return;
    }
    if (currentMode === 'capture' && !pickerOpen && !saveAsOpen) {
      focusEditor();
    }
  },
);

function handleKeydown(event: KeyboardEvent): void {
  if (event.isComposing || event.keyCode === 229) {
    return;
  }

  if (event.key === 'Escape') {
    event.preventDefault();
    void store.handleEscape();
    return;
  }

  if (store.isUnconfigured) {
    return;
  }

  if (
    store.mode === 'capture' &&
    event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey
  ) {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      store.cycleRelatedTarget('next');
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      store.cycleRelatedTarget('prev');
      return;
    }
  }

  actionRegistry.dispatchKeyEvent(event, {
    activeScope: 'quickbar',
    activeScopes: ['quickbar', 'global'],
  });
}
</script>

<template>
  <div class="qn-quickbar" @keydown="handleKeydown">
    <header class="qn-quickbar__header">
      <div class="qn-quickbar__modes">
        <button
          type="button"
          :class="['qn-quickbar__mode', { 'qn-quickbar__mode--active': store.mode === 'capture' }]"
          @click="store.mode = 'capture'"
        >
          记录
        </button>
        <button
          type="button"
          :class="['qn-quickbar__mode', { 'qn-quickbar__mode--active': store.mode === 'open' }]"
          @click="store.mode = 'open'"
        >
          打开 ⌘K
        </button>
        <button
          type="button"
          :class="['qn-quickbar__mode', { 'qn-quickbar__mode--active': store.mode === 'command' }]"
          @click="store.mode = 'command'"
        >
          命令 ⌘⇧K
        </button>
      </div>

      <div class="qn-quickbar__header-right">
        <button
          v-if="!store.isUnconfigured"
          type="button"
          class="qn-quickbar__chip-btn"
          data-testid="quickbar-target-chip"
          @click="store.toggleTargetPicker()"
        >
          <QnBadge :tone="store.targetChipTone">
            {{ store.targetChipLabel }}
          </QnBadge>
        </button>
        <QnBadge v-else tone="warning" data-testid="quickbar-target-chip">
          {{ store.targetChipLabel }}
        </QnBadge>

        <button
          v-if="store.explicitTarget && !store.isUnconfigured"
          type="button"
          class="qn-quickbar__clear-target"
          data-testid="quickbar-clear-explicit"
          @click="store.clearExplicitTarget()"
        >
          × 恢复默认
        </button>

        <span
          v-if="store.sectionHintText"
          class="qn-quickbar__section-hint"
          data-testid="quickbar-section-hint"
        >
          {{ store.sectionHintText }}
        </span>

        <button
          type="button"
          :class="[
            'qn-quickbar__aux-btn',
            { 'qn-quickbar__aux-btn--active': store.isAuxiliarySidebarActive },
          ]"
          :title="
            store.isAuxiliarySidebarActive
              ? '退出 45% 侧栏辅助窗口并恢复标准模式'
              : '在当前空间呼出 45% 侧栏辅助窗口'
          "
          :aria-pressed="store.isAuxiliarySidebarActive"
          data-testid="quickbar-toggle-auxiliary"
          @click="store.toggleAuxiliarySidebarMode()"
        >
          45% 侧栏
        </button>

        <button
          type="button"
          class="qn-quickbar__chip-btn"
          :class="{ 'qn-quickbar__pin-btn--active': store.isPinned }"
          :title="store.isPinned ? '已固定：失焦不关闭' : '固定后失焦不关闭'"
          :aria-pressed="store.isPinned"
          data-testid="quickbar-pin-toggle"
          @click="store.togglePin()"
        >
          📌
        </button>

        <QnButton variant="ghost" size="sm" @click="store.handleEscape()">Esc</QnButton>
      </div>
    </header>

    <QuickBarTargetPicker />

    <div v-if="store.settingsStatus && !store.settingsStatus.valid" class="qn-quickbar__banner">
      <QnBanner tone="danger" title="settings.yaml 解析异常">
        <span v-if="store.settingsStatus.errorLine !== null">
          第 {{ store.settingsStatus.errorLine }} 行
        </span>
        <span v-if="store.settingsStatus.errorColumn !== null">
          第 {{ store.settingsStatus.errorColumn }} 列：
        </span>
        {{ store.settingsStatus.errorMessage ?? '配置存在语法错误，已回退安全默认配置。' }}
      </QnBanner>
    </div>

    <div v-if="store.errorMessage" class="qn-quickbar__banner">
      <QnBanner tone="danger" title="追加失败">
        {{ store.errorMessage }}
      </QnBanner>
    </div>

    <div v-if="store.isUnconfigured" class="qn-quickbar__s1" data-testid="quickbar-s1-onboarding">
      <div class="qn-quickbar__s1-card">
        <QnBadge tone="warning" size="md">S1 未选库引导态</QnBadge>
        <h2 class="qn-quickbar__s1-title">请先选择或创建笔记库</h2>
        <p class="qn-quickbar__s1-desc">
          尚未配置本地笔记库根目录。当前已禁用输入与追加提交，请先在主窗口完成初始化。
        </p>
        <QnButton variant="primary" @click="store.openMainOnboarding()">
          打开主窗口完成初始化
        </QnButton>
      </div>
    </div>

    <div
      v-else-if="store.mode === 'open'"
      class="qn-quickbar__open-body"
      data-testid="quickbar-open-mode"
    >
      <div class="qn-quickbar__open-search">
        <button
          v-if="store.activeOpenDirectoryScope"
          type="button"
          class="qn-quickbar__open-scope-btn"
          data-testid="quickbar-open-back"
          @click="store.exitOpenDirectory()"
        >
          ← {{ store.activeOpenDirectoryScope.title }}
        </button>
        <input
          ref="openInputRef"
          type="text"
          class="qn-quickbar__open-input"
          data-testid="quickopen-input"
          :value="store.openQuery"
          :placeholder="
            store.activeOpenDirectoryScope
              ? '筛选目录文件（含隐藏文件），空输入按 Backspace 返回上级…'
              : '搜索笔记、外部书签，或输入 / 与 ~/ 路径直接回车打开…'
          "
          @input="handleOpenInput"
          @keydown="handleOpenKeydown"
        />
      </div>

      <div class="qn-quickbar__open-results" data-testid="quickbar-open-results">
        <div
          v-if="store.openError"
          class="qn-quickbar__open-empty"
          data-testid="quickbar-open-error"
        >
          {{ store.openError }}
        </div>
        <div
          v-else-if="store.openGroups.length === 0"
          class="qn-quickbar__open-empty"
          data-testid="quickbar-open-empty"
        >
          {{ store.openLoading ? '正在检索…' : '暂无匹配项，可输入关键字或 /、~/ 绝对路径' }}
        </div>
        <template v-else>
          <section
            v-for="(group, groupIdx) in store.openGroups"
            :key="group.id"
            class="qn-quickbar__open-group"
            :data-testid="`quickbar-open-group-${group.id}`"
          >
            <div class="qn-quickbar__open-group-title">{{ group.label }}</div>
            <button
              v-for="(item, itemIdx) in group.items"
              :key="item.id"
              type="button"
              :class="[
                'qn-quickbar__open-item',
                {
                  'qn-quickbar__open-item--active':
                    store.openSelectedIndex === getOpenItemFlatIndex(groupIdx, itemIdx),
                },
              ]"
              :data-testid="`quickbar-open-item-${item.group}-${itemIdx}`"
              @mouseenter="store.openSelectedIndex = getOpenItemFlatIndex(groupIdx, itemIdx)"
              @click="store.selectOpenItem(item)"
            >
              <div class="qn-quickbar__open-item-main">
                <span class="qn-quickbar__open-item-title">{{ item.title }}</span>
                <span class="qn-quickbar__open-item-sub">{{ item.subtitle }}</span>
              </div>
              <QnBadge :tone="getOpenBadgeTone(item.group)">
                {{ item.badgeLabel }}
              </QnBadge>
            </button>
          </section>
        </template>
      </div>
    </div>

    <div
      v-else-if="store.mode === 'command'"
      class="qn-quickbar__open-body"
      data-testid="quickbar-command-mode"
    >
      <CommandBookPalette
        :engine="commandPaletteEngine"
        variant="compact"
        :focus-trigger="commandFocusTick"
      />
    </div>

    <div v-else class="qn-quickbar__body">
      <textarea
        ref="editorRef"
        v-model="store.draft"
        class="qn-quickbar__editor"
        autofocus
        placeholder="输入要快速记录的想法、待办或片段…（⌘1 列表 / ⌘2 待办 / ⌘3 正文，⌘T 选目标）"
      />

      <div
        v-if="store.relatedNotes.length > 0"
        class="qn-quickbar__related"
        data-testid="quickbar-related-list"
      >
        <span class="qn-quickbar__related-label">相关笔记 (⌘↑/⌘↓ 改投):</span>
        <div class="qn-quickbar__related-items">
          <button
            v-for="(item, idx) in store.relatedNotes"
            :key="item.key"
            type="button"
            :class="[
              'qn-quickbar__related-item',
              { 'qn-quickbar__related-item--active': store.selectedRelatedIndex === idx },
            ]"
            :data-testid="`quickbar-related-item-${idx}`"
            @click="store.selectRelatedNote(store.selectedRelatedIndex === idx ? -1 : idx)"
          >
            {{ item.title }}
          </button>
        </div>
      </div>

      <form
        v-if="store.saveAsModalOpen"
        class="qn-quickbar__save-as"
        data-testid="quickbar-save-as-form"
        @submit.prevent="store.confirmSaveAsNote()"
      >
        <div class="qn-quickbar__save-as-fields">
          <input
            v-model="store.saveAsTitle"
            type="text"
            class="qn-quickbar__save-as-input"
            data-testid="quickbar-save-as-title"
            placeholder="新笔记标题"
          />
          <input
            v-model="store.saveAsDir"
            type="text"
            list="qn-quickbar-save-as-dirs"
            class="qn-quickbar__save-as-input"
            data-testid="quickbar-save-as-dir"
            placeholder="目标目录（留空为库根）"
          />
          <datalist id="qn-quickbar-save-as-dirs">
            <option v-for="dir in store.saveAsDirs" :key="dir" :value="dir" />
          </datalist>
          <select
            v-model="store.saveAsTemplate"
            class="qn-quickbar__save-as-select"
            data-testid="quickbar-save-as-template"
          >
            <option v-for="tpl in store.availableTemplates" :key="tpl" :value="tpl">
              {{ tpl }}
            </option>
          </select>
        </div>
        <div class="qn-quickbar__save-as-actions">
          <QnButton type="button" variant="ghost" size="sm" @click="store.closeSaveAsNoteDialog()">
            取消
          </QnButton>
          <QnButton
            type="submit"
            variant="primary"
            size="sm"
            :loading="store.submitting"
            data-testid="quickbar-save-as-confirm"
          >
            确认另存
          </QnButton>
        </div>
      </form>

      <footer class="qn-quickbar__footer">
        <div class="qn-quickbar__capture-modes">
          <button
            type="button"
            :class="[
              'qn-quickbar__capture-btn',
              { 'qn-quickbar__capture-btn--active': store.captureMode === 'list' },
            ]"
            data-testid="capture-mode-list"
            @click="store.setCaptureMode('list')"
          >
            列表 ⌘1
          </button>
          <button
            type="button"
            :class="[
              'qn-quickbar__capture-btn',
              { 'qn-quickbar__capture-btn--active': store.captureMode === 'todo' },
            ]"
            data-testid="capture-mode-todo"
            @click="store.setCaptureMode('todo')"
          >
            待办 ⌘2
          </button>
          <button
            type="button"
            :class="[
              'qn-quickbar__capture-btn',
              { 'qn-quickbar__capture-btn--active': store.captureMode === 'prose' },
            ]"
            data-testid="capture-mode-prose"
            @click="store.setCaptureMode('prose')"
          >
            正文 ⌘3
          </button>
        </div>

        <div class="qn-quickbar__footer-actions">
          <QnButton
            v-if="store.draft.length > 0"
            variant="ghost"
            size="sm"
            data-testid="quickbar-clear-draft"
            @click="store.clearDraft()"
          >
            清空
          </QnButton>
          <QnButton
            variant="secondary"
            size="sm"
            :disabled="!store.canSubmit"
            data-testid="quickbar-save-as-btn"
            @click="store.openSaveAsNoteDialog()"
          >
            ⌥⌘↵ 另存新笔记
          </QnButton>
          <QnButton
            variant="secondary"
            size="sm"
            :disabled="!store.canSubmit"
            data-testid="quickbar-submit-open"
            @click="store.submitAppend(true)"
          >
            追加并打开 ⇧⌘↵
          </QnButton>
          <QnButton
            variant="primary"
            size="sm"
            :disabled="!store.canSubmit"
            data-testid="quickbar-submit-append"
            @click="store.submitAppend(false)"
          >
            追加 ⌘↵
          </QnButton>
        </div>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.qn-quickbar {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-lg);
  overflow: hidden;
}

.qn-quickbar__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-3) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-quickbar__modes {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-quickbar__mode {
  padding: var(--qn-space-1) var(--qn-space-3);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 500;
  cursor: pointer;
}

.qn-quickbar__mode:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__mode--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__header-right {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-quickbar__chip-btn {
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.qn-quickbar__pin-btn--active {
  background-color: var(--qn-accent-soft);
  border: 1px solid var(--qn-accent);
  border-radius: var(--qn-radius-sm);
}

.qn-quickbar__clear-target {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-quickbar__clear-target:hover {
  color: var(--qn-text-primary);
}

.qn-quickbar__section-hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-quickbar__aux-btn {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 500;
  white-space: nowrap;
  cursor: pointer;
}

.qn-quickbar__aux-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__aux-btn--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__banner {
  padding: var(--qn-space-2) var(--qn-space-4) 0;
}

.qn-quickbar__s1,
.qn-quickbar__placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--qn-space-6);
}

.qn-quickbar__s1-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: var(--qn-space-3);
  max-width: 420px;
}

.qn-quickbar__s1-title {
  margin: 0;
  font-size: var(--qn-text-md);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-quickbar__s1-desc {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-quickbar__open-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  gap: var(--qn-space-2);
  padding: var(--qn-space-3);
  background-color: var(--qn-bg-elevated);
}

.qn-quickbar__open-search {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-default);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
}

.qn-quickbar__open-scope-btn {
  flex-shrink: 0;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-quickbar__open-scope-btn:hover {
  background-color: var(--qn-bg-hover);
}

.qn-quickbar__open-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-quickbar__open-input:focus {
  outline: none;
}

.qn-quickbar__open-results {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-quickbar__open-empty {
  padding: var(--qn-space-4);
  text-align: center;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-muted);
}

.qn-quickbar__open-group {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-quickbar__open-group-title {
  padding: 0 var(--qn-space-2);
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-muted);
}

.qn-quickbar__open-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  width: 100%;
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.qn-quickbar__open-item:hover,
.qn-quickbar__open-item--active {
  border-color: var(--qn-border-subtle);
  background-color: var(--qn-bg-active);
}

.qn-quickbar__open-item-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: var(--qn-space-1);
}

.qn-quickbar__open-item-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
}

.qn-quickbar__open-item-sub {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-quickbar__body {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.qn-quickbar__editor {
  flex: 1;
  width: 100%;
  padding: var(--qn-space-4);
  border: none;
  resize: none;
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-base);
  line-height: 1.6;
}

.qn-quickbar__editor:focus {
  outline: none;
}

.qn-quickbar__related {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
  overflow-x: auto;
}

.qn-quickbar__related-label {
  flex-shrink: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-quickbar__related-items {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-quickbar__related-item {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  white-space: nowrap;
  cursor: pointer;
}

.qn-quickbar__related-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__related-item--active {
  border-color: var(--qn-accent);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-quickbar__capture-modes {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-quickbar__capture-btn {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  cursor: pointer;
}

.qn-quickbar__capture-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-quickbar__capture-btn--active {
  border-color: var(--qn-border-default);
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-quickbar__footer-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-quickbar__save-as {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-quickbar__save-as-fields {
  display: flex;
  flex: 1;
  min-width: 0;
  gap: var(--qn-space-2);
}

.qn-quickbar__save-as-input,
.qn-quickbar__save-as-select {
  min-width: 0;
  flex: 1;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-xs);
}

.qn-quickbar__save-as-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex-shrink: 0;
}
</style>
