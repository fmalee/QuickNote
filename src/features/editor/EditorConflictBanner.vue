<script setup lang="ts">
import { computed } from 'vue';
import { QnBanner, QnButton } from '@/components';
import { useEditorTabsStore } from './store';

const editorTabsStore = useEditorTabsStore();

const activeTab = computed(() => editorTabsStore.activeTab);

const showModifiedBanner = computed(
  () =>
    activeTab.value?.conflictState?.kind === 'modified' &&
    !activeTab.value.conflictState.bannerDismissed &&
    !activeTab.value.conflictState.mergeMode,
);

const showDismissedConflictNotice = computed(
  () =>
    activeTab.value?.conflictState?.kind === 'modified' &&
    Boolean(activeTab.value.conflictState.bannerDismissed) &&
    !activeTab.value.conflictState.mergeMode,
);

const showDeletedBanner = computed(() => activeTab.value?.conflictState?.kind === 'deleted');
</script>

<template>
  <div v-if="showModifiedBanner && activeTab" class="qn-conflict-banner-wrap">
    <QnBanner tone="warning" title="检测到外部修改冲突" data-testid="editor-conflict-banner">
      磁盘文件已被外部程序修改，且当前标签存在未保存的编辑内容。
      <template #actions>
        <QnButton
          size="sm"
          variant="secondary"
          data-testid="conflict-reload-btn"
          @click="editorTabsStore.resolveConflictReload(activeTab.key)"
        >
          重新加载磁盘版本
        </QnButton>
        <QnButton
          size="sm"
          variant="primary"
          data-testid="conflict-overwrite-btn"
          @click="void editorTabsStore.resolveConflictOverwrite(activeTab.key)"
        >
          备份并覆盖磁盘
        </QnButton>
        <QnButton
          size="sm"
          variant="secondary"
          data-testid="conflict-merge-btn"
          @click="editorTabsStore.enterConflictMerge(activeTab.key)"
        >
          并排对比
        </QnButton>
        <QnButton
          size="sm"
          variant="ghost"
          data-testid="conflict-dismiss-btn"
          @click="editorTabsStore.dismissConflictBanner(activeTab.key)"
        >
          暂缓处理
        </QnButton>
      </template>
    </QnBanner>
  </div>

  <div v-else-if="showDismissedConflictNotice && activeTab" class="qn-conflict-banner-wrap">
    <QnBanner
      tone="warning"
      title="自动保存已暂停（存在未处理冲突）"
      data-testid="editor-conflict-paused-banner"
    >
      磁盘冲突解决前，当前编辑内容不会自动写入磁盘。
      <template #actions>
        <QnButton
          size="sm"
          variant="primary"
          data-testid="conflict-reopen-btn"
          @click="editorTabsStore.reopenConflictBanner(activeTab.key)"
        >
          展开处理冲突
        </QnButton>
      </template>
    </QnBanner>
  </div>

  <div v-else-if="showDeletedBanner && activeTab" class="qn-conflict-banner-wrap">
    <QnBanner tone="danger" title="磁盘文件已丢失" data-testid="editor-deleted-banner">
      磁盘上的文件已被外部删除，当前未保存的正文仍保留在编辑器中。
      <template #actions>
        <QnButton
          size="sm"
          variant="primary"
          data-testid="deleted-restore-btn"
          @click="void editorTabsStore.restoreDeletedTabToDisk(activeTab.key)"
        >
          写回原路径
        </QnButton>
        <QnButton
          size="sm"
          variant="secondary"
          data-testid="deleted-close-btn"
          @click="editorTabsStore.closeTab(activeTab.key)"
        >
          关闭标签
        </QnButton>
      </template>
    </QnBanner>
  </div>
</template>

<style scoped>
.qn-conflict-banner-wrap {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
}
</style>
