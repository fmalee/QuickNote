<script setup lang="ts">
export interface QnBannerProps {
  tone?: 'info' | 'warning' | 'danger' | 'success';
  title?: string;
}

withDefaults(defineProps<QnBannerProps>(), {
  tone: 'info',
  title: undefined,
});
</script>

<template>
  <div :class="['qn-banner', `qn-banner--${tone}`]" role="status">
    <div class="qn-banner__body">
      <div v-if="title" class="qn-banner__title">{{ title }}</div>
      <div class="qn-banner__content">
        <slot />
      </div>
    </div>
    <div v-if="$slots.actions" class="qn-banner__actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.qn-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-2) var(--qn-space-4);
  border-radius: var(--qn-radius-sm);
  border: 1px solid var(--qn-border-subtle);
  font-size: var(--qn-text-sm);
}

.qn-banner--info {
  background-color: var(--qn-accent-soft);
  border-color: var(--qn-accent);
  color: var(--qn-text-primary);
}

.qn-banner--warning {
  background-color: var(--qn-warning-soft);
  border-color: var(--qn-warning-border);
  color: var(--qn-text-primary);
}

.qn-banner--danger {
  background-color: var(--qn-danger-soft);
  border-color: var(--qn-danger-border);
  color: var(--qn-text-primary);
}

.qn-banner--success {
  background-color: var(--qn-success-soft);
  border-color: var(--qn-success-border);
  color: var(--qn-text-primary);
}

.qn-banner__body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.qn-banner__title {
  font-weight: 600;
  font-size: var(--qn-text-sm);
}

.qn-banner__content {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  word-break: break-word;
}

.qn-banner__actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}
</style>
