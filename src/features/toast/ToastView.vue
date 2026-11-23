<script setup lang="ts">
import { computed } from 'vue';
import { QnBadge, QnButton } from '@/components';
import { useToastStore } from './store';

const store = useToastStore();

const targetTitle = computed<string>(() => {
  const current = store.payload;
  if (!current) {
    return '';
  }
  return current.title || current.relPath;
});
</script>

<template>
  <div class="qn-toast-viewport">
    <Transition name="qn-toast-fade">
      <div v-if="store.visible && store.payload" class="qn-toast-pill" role="status">
        <span class="qn-toast-pill__dot" aria-hidden="true" />
        <div class="qn-toast-pill__text">
          <span class="qn-toast-pill__title">已追加至「{{ targetTitle }}」</span>
          <span v-if="store.payload.sectionHeading" class="qn-toast-pill__meta">
            {{ store.payload.sectionHeading }}
          </span>
        </div>
        <QnBadge tone="neutral" size="sm">{{ store.remainingSeconds }}s</QnBadge>
        <QnButton
          v-if="store.payload.undoAvailable !== false"
          variant="secondary"
          size="sm"
          @click="store.triggerUndo()"
        >
          撤销
        </QnButton>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.qn-toast-viewport {
  display: flex;
  align-items: stretch;
  justify-content: stretch;
  width: 100%;
  height: 100%;
  padding: 0;
  background-color: transparent;
}

.qn-toast-pill {
  z-index: var(--qn-z-toast);
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  width: 100%;
  height: 100%;
  padding: 0 var(--qn-space-3);
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  box-sizing: border-box;
}

.qn-toast-pill__dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--qn-success);
  flex-shrink: 0;
}

.qn-toast-pill__text {
  flex: 1;
  display: flex;
  align-items: baseline;
  gap: var(--qn-space-2);
  min-width: 0;
}

.qn-toast-pill__title {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.qn-toast-pill__meta {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
  white-space: nowrap;
}

.qn-toast-fade-enter-active,
.qn-toast-fade-leave-active {
  transition:
    opacity 0.18s ease,
    transform 0.18s ease;
}

.qn-toast-fade-enter-from,
.qn-toast-fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
</style>
