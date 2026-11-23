<script setup lang="ts">
import { QnBanner } from '@/components';
import { useEditorTabsStore } from './store';

const editorTabsStore = useEditorTabsStore();
</script>

<template>
  <div v-if="editorTabsStore.isInboxBloated" class="qn-inbox-bloat-banner">
    <QnBanner tone="warning" data-testid="inbox-bloat-banner">
      收件箱已超过 {{ editorTabsStore.inboxBloatThreshold }} 行，建议整理
    </QnBanner>
  </div>
</template>

<style scoped>
.qn-inbox-bloat-banner {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
}
</style>
