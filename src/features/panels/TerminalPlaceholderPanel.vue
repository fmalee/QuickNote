<script setup lang="ts">
import { computed } from 'vue';
import { QnButton } from '@/components';
import { useEditorTabsStore } from '@/features/editor';
import { actionRegistry } from '@/lib/actions';

const editorTabsStore = useEditorTabsStore();

const hasActiveDoc = computed<boolean>(() => Boolean(editorTabsStore.activeDoc));

async function openExternalTerminal(): Promise<void> {
  const activeDoc = editorTabsStore.activeDoc;
  const relPath = activeDoc?.relPath ?? '';
  await actionRegistry.executeById('external.terminal', {
    activeScope: 'main-window',
    activeScopes: ['main-window', 'global'],
    hasActiveDoc: Boolean(activeDoc),
    hasSelection: Boolean(editorTabsStore.activeTab?.hasSelection),
    isMarkdown: relPath.toLowerCase().endsWith('.md'),
    isInbox: relPath === 'inbox.md',
  });
}
</script>

<template>
  <div class="qn-terminal-placeholder" data-testid="terminal-placeholder-panel">
    <div class="qn-terminal-placeholder__card">
      <div class="qn-terminal-placeholder__title">内嵌终端（M4 阶段启用）</div>
      <p class="qn-terminal-placeholder__desc">
        内嵌 PTY 终端将在 M4 阶段启用，当前可按 ⌥⌘T 在外部终端打开项目目录。
      </p>
      <QnButton
        variant="secondary"
        size="sm"
        :disabled="!hasActiveDoc"
        data-testid="terminal-open-external-btn"
        @click="openExternalTerminal"
      >
        在外部终端打开 (⌥⌘T)
      </QnButton>
    </div>
  </div>
</template>

<style scoped>
.qn-terminal-placeholder {
  flex: 1;
  min-height: 0;
  padding: var(--qn-space-3);
  display: flex;
  flex-direction: column;
}

.qn-terminal-placeholder__card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--qn-space-2);
  padding: var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  background-color: var(--qn-bg-base);
}

.qn-terminal-placeholder__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-terminal-placeholder__desc {
  margin: 0;
  font-size: var(--qn-text-xs);
  line-height: 1.5;
  color: var(--qn-text-secondary);
}
</style>
