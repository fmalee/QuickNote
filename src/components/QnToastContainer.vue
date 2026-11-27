<script setup lang="ts">
export interface ToastItem {
  id: number;
  tone: 'info' | 'success' | 'warning' | 'danger';
  title?: string;
  message: string;
}

defineProps<{
  items: readonly ToastItem[];
}>();

const emit = defineEmits<{
  (e: 'dismiss', id: number): void;
  (e: 'pause', id: number): void;
  (e: 'resume', id: number): void;
}>();
</script>

<template>
  <div
    v-if="items.length > 0"
    class="qn-toast-container"
    data-testid="workbench-toast-container"
    aria-live="polite"
  >
    <TransitionGroup name="qn-toast-stack" tag="div" class="qn-toast-container__list">
      <div
        v-for="item in items"
        :key="item.id"
        :class="['qn-toast-card', `qn-toast-card--${item.tone}`]"
        :role="item.tone === 'danger' ? 'alert' : 'status'"
        data-testid="workbench-toast-item"
        @mouseenter="emit('pause', item.id)"
        @mouseleave="emit('resume', item.id)"
      >
        <span class="qn-toast-card__dot" aria-hidden="true" />
        <div class="qn-toast-card__body">
          <strong v-if="item.title" class="qn-toast-card__title">{{ item.title }}</strong>
          <span class="qn-toast-card__message">{{ item.message }}</span>
        </div>
        <button
          type="button"
          class="qn-toast-card__close"
          data-testid="workbench-toast-close"
          aria-label="关闭通知"
          @click="emit('dismiss', item.id)"
        >
          ×
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.qn-toast-container {
  position: fixed;
  right: var(--qn-space-4);
  bottom: calc(var(--qn-space-7) + var(--qn-space-2));
  z-index: var(--qn-z-toast);
  max-width: min(380px, calc(100vw - var(--qn-space-8)));
  pointer-events: none;
}

.qn-toast-container__list {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-toast-card {
  pointer-events: auto;
  display: flex;
  align-items: flex-start;
  gap: var(--qn-space-2);
  padding: var(--qn-space-2) var(--qn-space-3);
  border-radius: var(--qn-radius-md);
  border: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  box-shadow: var(--qn-shadow-md);
  font-size: var(--qn-text-sm);
  line-height: 1.45;
}

.qn-toast-card--info {
  border-color: var(--qn-border-strong);
}

.qn-toast-card--success {
  border-color: var(--qn-success-border);
}

.qn-toast-card--warning {
  border-color: var(--qn-warning-border);
}

.qn-toast-card--danger {
  border-color: var(--qn-danger-border);
}

.qn-toast-card__dot {
  width: 8px;
  height: 8px;
  margin-top: 5px;
  border-radius: var(--qn-radius-full);
  flex-shrink: 0;
  background-color: var(--qn-accent);
}

.qn-toast-card--success .qn-toast-card__dot {
  background-color: var(--qn-success);
}

.qn-toast-card--warning .qn-toast-card__dot {
  background-color: var(--qn-warning);
}

.qn-toast-card--danger .qn-toast-card__dot {
  background-color: var(--qn-danger);
}

.qn-toast-card__body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-toast-card__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-toast-card__message {
  color: var(--qn-text-secondary);
  word-break: break-word;
}

.qn-toast-card__close {
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  cursor: pointer;
  padding: 0 var(--qn-space-1);
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-base);
  line-height: 1.2;
  flex-shrink: 0;
}

.qn-toast-card__close:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-toast-stack-enter-active,
.qn-toast-stack-leave-active {
  transition:
    opacity 0.18s ease,
    transform 0.18s ease;
}

.qn-toast-stack-enter-from,
.qn-toast-stack-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
</style>
