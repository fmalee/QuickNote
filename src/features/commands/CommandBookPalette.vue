<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import type { CommandEntry } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton } from '@/components';
import { ALL_SECTIONS_KEY, type CommandPaletteEngine } from './useCommandPalette';

export interface CommandBookPaletteProps {
  engine: CommandPaletteEngine;
  variant?: 'compact' | 'large';
  focusTrigger?: number;
}

const props = withDefaults(defineProps<CommandBookPaletteProps>(), {
  variant: 'compact',
  focusTrigger: 0,
});

const emit = defineEmits<{
  close: [];
}>();

const searchInputRef = ref<HTMLInputElement | null>(null);
const manualDirInputRef = ref<HTMLInputElement | null>(null);

interface CompactGroupItem {
  entry: CommandEntry;
  flatIndex: number;
}

interface CompactGroup {
  section: string;
  items: CompactGroupItem[];
}

const compactGroups = computed<CompactGroup[]>(() => {
  const list = props.engine.filteredEntries.value;
  const groups = new Map<string, CompactGroupItem[]>();
  list.forEach((entry, flatIndex) => {
    const section = entry.section || '未分区';
    const existing = groups.get(section);
    if (existing) {
      existing.push({ entry, flatIndex });
    } else {
      groups.set(section, [{ entry, flatIndex }]);
    }
  });
  return Array.from(groups.entries()).map(([section, items]) => ({ section, items }));
});

const terminalStatusText = computed<string>(() => {
  const state = props.engine.terminalState.value;
  if (state?.enabled && state.resolvedPath) {
    return `将在 ${state.resolvedPath} 打开外部终端`;
  }
  return '未关联项目目录 · 按 ⌘↵ 手动选择';
});

function focusSearchInput(): void {
  void nextTick(() => {
    searchInputRef.value?.focus();
    searchInputRef.value?.select();
  });
}

onMounted(() => {
  focusSearchInput();
});

watch(
  () => props.focusTrigger,
  () => {
    focusSearchInput();
  },
);

watch(
  () => props.engine.dirPickerOpen.value,
  (open) => {
    if (open) {
      void nextTick(() => {
        manualDirInputRef.value?.focus();
      });
    } else {
      focusSearchInput();
    }
  },
);

function handleSearchInput(event: Event): void {
  const target = event.target as HTMLInputElement;
  props.engine.setQuery(target.value);
}

function handleManualDirInput(event: Event): void {
  const target = event.target as HTMLInputElement;
  props.engine.setManualDirInput(target.value);
}

function handleSelectEntry(index: number): void {
  props.engine.selectIndex(index);
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.isComposing || event.keyCode === 229) {
    return;
  }

  if (props.engine.dirPickerOpen.value) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      props.engine.closeDirPicker();
      return;
    }
    if (event.key === 'Enter' && !event.metaKey && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      event.stopPropagation();
      void props.engine.confirmManualDir();
      return;
    }
    return;
  }

  if (event.key === 'Escape') {
    if (props.variant === 'large') {
      event.preventDefault();
      event.stopPropagation();
      emit('close');
      void props.engine.requestClose();
    }
    return;
  }

  if (event.key === 'ArrowDown') {
    event.preventDefault();
    props.engine.moveSelection(1);
    return;
  }

  if (event.key === 'ArrowUp') {
    event.preventDefault();
    props.engine.moveSelection(-1);
    return;
  }

  if (
    event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'o'
  ) {
    event.preventDefault();
    event.stopPropagation();
    void props.engine.openSourceFile();
    return;
  }

  if (
    event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey &&
    event.key === 'Enter'
  ) {
    event.preventDefault();
    event.stopPropagation();
    void props.engine.copyAndOpenTerminal();
    return;
  }

  if (
    !event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey &&
    event.key === 'Enter'
  ) {
    event.preventDefault();
    event.stopPropagation();
    void props.engine.copySelected();
  }
}
</script>

<template>
  <div
    :class="['qn-cmd-palette', `qn-cmd-palette--${variant}`]"
    data-testid="command-book-palette"
    @keydown="handleKeydown"
  >
    <div class="qn-cmd-palette__search-bar">
      <input
        ref="searchInputRef"
        type="text"
        class="qn-cmd-palette__search-input"
        data-testid="cmd-search-input"
        :value="engine.query.value"
        placeholder="搜索命令、描述、关键字或分区…（↵ 复制 / ⌘↵ 开终端 / ⌘O 源文件）"
        @input="handleSearchInput"
      />
      <div class="qn-cmd-palette__search-meta">
        <QnBadge
          v-if="engine.feedbackMessage.value"
          tone="success"
          data-testid="cmd-feedback-badge"
        >
          {{ engine.feedbackMessage.value }}
        </QnBadge>
        <span class="qn-cmd-palette__count"> {{ engine.filteredEntries.value.length }} 条 </span>
      </div>
    </div>

    <div v-if="engine.errorMessage.value" class="qn-cmd-palette__banner">
      <QnBanner tone="danger" title="命令操作失败">
        {{ engine.errorMessage.value }}
      </QnBanner>
    </div>

    <div v-if="engine.dirPickerOpen.value" class="qn-cmd-dir-picker" data-testid="cmd-dir-picker">
      <div class="qn-cmd-dir-picker__header">
        <strong>选择外部终端目标目录</strong>
        <span class="qn-cmd-dir-picker__hint">
          未自动关联项目目录，已复制命令。请选择工作区或输入路径后打开外部终端：
        </span>
      </div>

      <div v-if="engine.workspaceOptions.value.length > 0" class="qn-cmd-dir-picker__workspaces">
        <button
          v-for="ws in engine.workspaceOptions.value"
          :key="ws.key"
          type="button"
          class="qn-cmd-dir-picker__ws-btn"
          :data-testid="`cmd-dir-ws-${ws.key}`"
          @click="engine.confirmManualDir(ws.dirPath)"
        >
          <span class="qn-cmd-dir-picker__ws-name">{{ ws.name }}</span>
          <span class="qn-cmd-dir-picker__ws-path">{{ ws.dirPath }}</span>
        </button>
      </div>

      <div class="qn-cmd-dir-picker__manual">
        <input
          ref="manualDirInputRef"
          :value="engine.manualDirInput.value"
          type="text"
          class="qn-cmd-dir-picker__input"
          data-testid="cmd-dir-manual-input"
          placeholder="输入项目目录绝对路径，如 /Users/.../project"
          @input="handleManualDirInput"
        />
        <QnButton
          variant="primary"
          size="sm"
          data-testid="cmd-dir-confirm-btn"
          @click="engine.confirmManualDir()"
        >
          打开终端
        </QnButton>
        <QnButton
          variant="ghost"
          size="sm"
          data-testid="cmd-dir-cancel-btn"
          @click="engine.closeDirPicker()"
        >
          取消
        </QnButton>
      </div>

      <p v-if="engine.dirPickerError.value" class="qn-cmd-dir-picker__error">
        {{ engine.dirPickerError.value }}
      </p>
    </div>

    <!-- Compact Variant (Quick Bar 命令模式) -->
    <div v-if="variant === 'compact'" class="qn-cmd-compact__body">
      <div
        v-if="engine.filteredEntries.value.length === 0"
        class="qn-cmd-palette__empty"
        data-testid="cmd-empty"
      >
        {{ engine.loading.value ? '正在检索命令…' : '未找到匹配的命令条目' }}
      </div>

      <div v-else class="qn-cmd-compact__groups">
        <section v-for="group in compactGroups" :key="group.section" class="qn-cmd-compact__group">
          <header class="qn-cmd-compact__group-title">{{ group.section }}</header>
          <div
            v-for="item in group.items"
            :key="item.entry.id"
            :class="[
              'qn-cmd-item',
              {
                'qn-cmd-item--selected': item.flatIndex === engine.selectedIndex.value,
                'qn-cmd-item--danger': item.entry.isDanger,
              },
            ]"
            :data-testid="`cmd-item-${item.entry.id}`"
            @click="handleSelectEntry(item.flatIndex)"
          >
            <div class="qn-cmd-item__top">
              <div class="qn-cmd-item__title-row">
                <span class="qn-cmd-item__title">{{ item.entry.title }}</span>
                <QnBadge v-if="item.entry.isDanger" tone="danger" data-testid="cmd-danger-badge">
                  ⚠ 危险
                </QnBadge>
                <QnBadge v-if="item.entry.language" tone="neutral">
                  {{ item.entry.language }}
                </QnBadge>
              </div>
              <div class="qn-cmd-item__actions">
                <QnButton
                  variant="secondary"
                  size="sm"
                  data-testid="cmd-copy-btn"
                  @click.stop="
                    handleSelectEntry(item.flatIndex);
                    engine.copySelected();
                  "
                >
                  ↵ 复制
                </QnButton>
                <QnButton
                  variant="primary"
                  size="sm"
                  data-testid="cmd-terminal-btn"
                  @click.stop="
                    handleSelectEntry(item.flatIndex);
                    engine.copyAndOpenTerminal();
                  "
                >
                  ⌘↵ 开终端
                </QnButton>
              </div>
            </div>
            <p v-if="item.entry.description" class="qn-cmd-item__desc">
              {{ item.entry.description }}
            </p>
            <pre class="qn-cmd-item__code"><code>{{ item.entry.code }}</code></pre>
          </div>
        </section>
      </div>
    </div>

    <!-- Large Variant (主窗口大尺寸命令库) -->
    <div v-else class="qn-cmd-large__body">
      <aside class="qn-cmd-large__sidebar" data-testid="cmd-section-sidebar">
        <button
          type="button"
          :class="[
            'qn-cmd-large__section-btn',
            {
              'qn-cmd-large__section-btn--active':
                engine.selectedSection.value === ALL_SECTIONS_KEY,
            },
          ]"
          data-testid="cmd-section-all"
          @click="engine.selectSection(ALL_SECTIONS_KEY)"
        >
          <span>全部</span>
          <span class="qn-cmd-large__section-count">{{ engine.entries.value.length }}</span>
        </button>
        <button
          v-for="sec in engine.sections.value"
          :key="sec.name"
          type="button"
          :class="[
            'qn-cmd-large__section-btn',
            { 'qn-cmd-large__section-btn--active': engine.selectedSection.value === sec.name },
          ]"
          :data-testid="`cmd-section-${sec.name}`"
          @click="engine.selectSection(sec.name)"
        >
          <span>{{ sec.name }}</span>
          <span class="qn-cmd-large__section-count">{{ sec.count }}</span>
        </button>
      </aside>

      <div class="qn-cmd-large__list">
        <div
          v-if="engine.filteredEntries.value.length === 0"
          class="qn-cmd-palette__empty"
          data-testid="cmd-empty"
        >
          {{ engine.loading.value ? '正在检索命令…' : '未找到匹配的命令条目' }}
        </div>
        <button
          v-for="(entry, index) in engine.filteredEntries.value"
          :key="entry.id"
          type="button"
          :class="[
            'qn-cmd-large__list-item',
            {
              'qn-cmd-large__list-item--selected': index === engine.selectedIndex.value,
              'qn-cmd-large__list-item--danger': entry.isDanger,
            },
          ]"
          :data-testid="`cmd-item-${entry.id}`"
          @click="handleSelectEntry(index)"
        >
          <div class="qn-cmd-large__list-top">
            <span class="qn-cmd-item__title">{{ entry.title }}</span>
            <QnBadge v-if="entry.isDanger" tone="danger" data-testid="cmd-danger-badge">
              ⚠ 危险
            </QnBadge>
          </div>
          <span class="qn-cmd-large__list-sub">
            {{ entry.section }} · {{ entry.noteRelPath }}:{{ entry.line }}
          </span>
        </button>
      </div>

      <section class="qn-cmd-large__detail" data-testid="cmd-detail-pane">
        <template v-if="engine.selectedEntry.value">
          <header class="qn-cmd-large__detail-header">
            <div>
              <h3 class="qn-cmd-large__detail-title">{{ engine.selectedEntry.value.title }}</h3>
              <span class="qn-cmd-large__detail-meta">
                {{ engine.selectedEntry.value.section }} ·
                {{ engine.selectedEntry.value.noteRelPath }}:{{ engine.selectedEntry.value.line }}
              </span>
            </div>
            <div class="qn-cmd-large__detail-badges">
              <QnBadge
                v-if="engine.selectedEntry.value.isDanger"
                tone="danger"
                size="md"
                data-testid="cmd-detail-danger-badge"
              >
                ⚠ 危险命令
              </QnBadge>
              <QnBadge v-if="engine.selectedEntry.value.language" tone="neutral" size="md">
                {{ engine.selectedEntry.value.language }}
              </QnBadge>
            </div>
          </header>

          <QnBanner v-if="engine.selectedEntry.value.isDanger" tone="danger" title="危险操作警示">
            该命令包含不可逆或高风险操作，执行前请仔细核对当前工作目录与参数。
          </QnBanner>

          <p v-if="engine.selectedEntry.value.description" class="qn-cmd-large__detail-desc">
            {{ engine.selectedEntry.value.description }}
          </p>

          <pre
            class="qn-cmd-large__detail-code"
          ><code>{{ engine.selectedEntry.value.code }}</code></pre>

          <div v-if="engine.selectedEntry.value.keywords.length > 0" class="qn-cmd-large__keywords">
            <QnBadge v-for="kw in engine.selectedEntry.value.keywords" :key="kw" tone="neutral">
              {{ kw }}
            </QnBadge>
          </div>

          <div class="qn-cmd-large__detail-actions">
            <QnButton
              variant="secondary"
              size="sm"
              data-testid="cmd-detail-copy-btn"
              @click="engine.copySelected()"
            >
              ↵ 仅复制命令
            </QnButton>
            <QnButton
              variant="primary"
              size="sm"
              data-testid="cmd-detail-terminal-btn"
              @click="engine.copyAndOpenTerminal()"
            >
              ⌘↵ 复制并开外部终端
            </QnButton>
            <QnButton
              variant="ghost"
              size="sm"
              data-testid="cmd-detail-source-btn"
              @click="engine.openSourceFile()"
            >
              ⌘O 跳回源文件编辑
            </QnButton>
          </div>
        </template>
        <div v-else class="qn-cmd-palette__empty">请选择一条命令查看详情</div>
      </section>
    </div>

    <footer class="qn-cmd-palette__footer">
      <span class="qn-cmd-palette__terminal-status" data-testid="cmd-terminal-status">
        {{ terminalStatusText }}
      </span>
      <div class="qn-cmd-palette__footer-actions">
        <button
          type="button"
          class="qn-cmd-palette__source-link"
          data-testid="cmd-open-source-btn"
          :disabled="!engine.selectedEntry.value"
          @click="engine.openSourceFile()"
        >
          ⌘O 源文件
        </button>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.qn-cmd-palette {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  color: var(--qn-text-primary);
  min-height: 0;
}

.qn-cmd-palette--compact {
  max-height: 460px;
}

.qn-cmd-palette--large {
  height: 540px;
}

.qn-cmd-palette__search-bar {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
}

.qn-cmd-palette__search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--qn-text-primary);
  font-size: var(--qn-text-base);
  font-family: var(--qn-font-sans);
}

.qn-cmd-palette__search-input::placeholder {
  color: var(--qn-text-muted);
}

.qn-cmd-palette__search-meta {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}

.qn-cmd-palette__count {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-cmd-palette__banner {
  flex-shrink: 0;
}

.qn-cmd-dir-picker {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-warning-border);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-warning-soft);
}

.qn-cmd-dir-picker__header {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  font-size: var(--qn-text-sm);
}

.qn-cmd-dir-picker__hint {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-cmd-dir-picker__workspaces {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}

.qn-cmd-dir-picker__ws-btn {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  cursor: pointer;
  text-align: left;
}

.qn-cmd-dir-picker__ws-btn:hover {
  background-color: var(--qn-bg-hover);
  border-color: var(--qn-accent);
}

.qn-cmd-dir-picker__ws-name {
  font-size: var(--qn-text-xs);
  font-weight: 600;
}

.qn-cmd-dir-picker__ws-path {
  font-size: var(--qn-text-xs);
  font-family: var(--qn-font-mono);
  color: var(--qn-text-muted);
}

.qn-cmd-dir-picker__manual {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-cmd-dir-picker__input {
  flex: 1;
  min-width: 0;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-sm);
}

.qn-cmd-dir-picker__error {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}

.qn-cmd-palette__empty {
  padding: var(--qn-space-6);
  text-align: center;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-sm);
}

.qn-cmd-compact__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.qn-cmd-compact__groups {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-cmd-compact__group {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-cmd-compact__group-title {
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-muted);
  padding: 0 var(--qn-space-1);
}

.qn-cmd-item {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
  cursor: pointer;
}

.qn-cmd-item:hover {
  background-color: var(--qn-bg-hover);
}

.qn-cmd-item--selected {
  border-color: var(--qn-accent);
  background-color: var(--qn-accent-soft);
}

.qn-cmd-item--danger {
  border-color: var(--qn-danger);
  background-color: var(--qn-danger-soft);
}

.qn-cmd-item--danger.qn-cmd-item--selected {
  box-shadow: 0 0 0 1px var(--qn-danger);
}

.qn-cmd-item__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
}

.qn-cmd-item__title-row {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  min-width: 0;
}

.qn-cmd-item__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-cmd-item__actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex-shrink: 0;
}

.qn-cmd-item__desc {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-cmd-item__code {
  margin: 0;
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-sidebar);
  border: 1px solid var(--qn-border-subtle);
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.qn-cmd-large__body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 180px 260px 1fr;
  gap: var(--qn-space-3);
}

.qn-cmd-large__sidebar {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-sidebar);
  overflow-y: auto;
}

.qn-cmd-large__section-btn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-sm);
  cursor: pointer;
  text-align: left;
}

.qn-cmd-large__section-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-cmd-large__section-btn--active {
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
  font-weight: 600;
}

.qn-cmd-large__section-count {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-cmd-large__list {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
  overflow-y: auto;
}

.qn-cmd-large__list-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--qn-space-2);
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.qn-cmd-large__list-item:hover {
  background-color: var(--qn-bg-hover);
}

.qn-cmd-large__list-item--selected {
  border-color: var(--qn-accent);
  background-color: var(--qn-accent-soft);
}

.qn-cmd-large__list-item--danger {
  border-color: var(--qn-danger-border);
  background-color: var(--qn-danger-soft);
}

.qn-cmd-large__list-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-1);
}

.qn-cmd-large__list-sub {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-cmd-large__detail {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
  overflow-y: auto;
}

.qn-cmd-large__detail-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--qn-space-2);
}

.qn-cmd-large__detail-title {
  margin: 0;
  font-size: var(--qn-text-md);
  font-weight: 600;
}

.qn-cmd-large__detail-meta {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-cmd-large__detail-badges {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  flex-shrink: 0;
}

.qn-cmd-large__detail-desc {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
  line-height: 1.5;
}

.qn-cmd-large__detail-code {
  margin: 0;
  padding: var(--qn-space-3);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-sidebar);
  border: 1px solid var(--qn-border-subtle);
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.qn-cmd-large__keywords {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-1);
}

.qn-cmd-large__detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
  margin-top: auto;
}

.qn-cmd-palette__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-3);
  border-top: 1px solid var(--qn-border-subtle);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-cmd-palette__terminal-status {
  font-family: var(--qn-font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-cmd-palette__source-link {
  border: none;
  background: transparent;
  color: var(--qn-accent);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  padding: 2px var(--qn-space-1);
}

.qn-cmd-palette__source-link:disabled {
  color: var(--qn-text-muted);
  cursor: not-allowed;
}
</style>
