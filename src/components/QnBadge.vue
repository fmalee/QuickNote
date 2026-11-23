<script setup lang="ts">
export interface QnBadgeProps {
  tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger';
  size?: 'sm' | 'md';
}

withDefaults(defineProps<QnBadgeProps>(), {
  tone: 'neutral',
  size: 'sm',
});
</script>

<template>
  <span :class="['qn-badge', `qn-badge--${tone}`, `qn-badge--${size}`]">
    <slot />
  </span>
</template>

<style scoped>
.qn-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--qn-space-1);
  border-radius: var(--qn-radius-sm);
  font-weight: 500;
  line-height: 1.2;
  white-space: nowrap;
  border: 1px solid transparent;
}

.qn-badge--sm {
  padding: 2px var(--qn-space-2);
  font-size: var(--qn-text-xs);
}

.qn-badge--md {
  padding: var(--qn-space-1) var(--qn-space-2);
  font-size: var(--qn-text-sm);
}

.qn-badge--neutral {
  background-color: var(--qn-bg-sidebar);
  border-color: var(--qn-border-subtle);
  color: var(--qn-text-secondary);
}

.qn-badge--accent {
  background-color: var(--qn-accent-soft);
  border-color: var(--qn-accent);
  color: var(--qn-accent);
}

.qn-badge--success {
  background-color: var(--qn-success-soft);
  border-color: var(--qn-success-border);
  color: var(--qn-success);
}

.qn-badge--warning {
  background-color: var(--qn-warning-soft);
  border-color: var(--qn-warning-border);
  color: var(--qn-warning);
}

.qn-badge--danger {
  background-color: var(--qn-danger-soft);
  border-color: var(--qn-danger-border);
  color: var(--qn-danger);
}
</style>
