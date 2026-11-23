<script setup lang="ts">
export interface QnButtonProps {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md';
  disabled?: boolean;
  loading?: boolean;
  type?: 'button' | 'submit' | 'reset';
}

withDefaults(defineProps<QnButtonProps>(), {
  variant: 'secondary',
  size: 'md',
  disabled: false,
  loading: false,
  type: 'button',
});
</script>

<template>
  <button
    :type="type"
    :disabled="disabled || loading"
    :class="['qn-btn', `qn-btn--${variant}`, `qn-btn--${size}`]"
  >
    <span v-if="loading" class="qn-btn__spinner" aria-hidden="true" />
    <slot />
  </button>
</template>

<style scoped>
.qn-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--qn-space-2);
  border: 1px solid transparent;
  border-radius: var(--qn-radius-sm);
  font-weight: 500;
  line-height: 1;
  cursor: pointer;
  white-space: nowrap;
  user-select: none;
  transition:
    background-color 0.14s ease,
    border-color 0.14s ease,
    color 0.14s ease,
    opacity 0.14s ease;
}

.qn-btn--sm {
  height: 28px;
  padding: 0 var(--qn-space-3);
  font-size: var(--qn-text-xs);
}

.qn-btn--md {
  height: 34px;
  padding: 0 var(--qn-space-4);
  font-size: var(--qn-text-sm);
}

.qn-btn--primary {
  background-color: var(--qn-accent);
  color: var(--qn-accent-contrast);
  border-color: var(--qn-accent);
}

.qn-btn--primary:hover:not(:disabled) {
  background-color: var(--qn-accent-hover);
  border-color: var(--qn-accent-hover);
}

.qn-btn--secondary {
  background-color: var(--qn-bg-elevated);
  color: var(--qn-text-primary);
  border-color: var(--qn-border-strong);
}

.qn-btn--secondary:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
}

.qn-btn--ghost {
  background-color: transparent;
  color: var(--qn-text-secondary);
}

.qn-btn--ghost:hover:not(:disabled) {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-btn--danger {
  background-color: var(--qn-danger);
  color: var(--qn-accent-contrast);
  border-color: var(--qn-danger);
}

.qn-btn--danger:hover:not(:disabled) {
  opacity: 0.9;
}

.qn-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.qn-btn:focus-visible {
  outline: 2px solid var(--qn-accent);
  outline-offset: 2px;
}

.qn-btn__spinner {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 2px solid currentColor;
  border-right-color: transparent;
  animation: qn-spin 0.7s linear infinite;
}

@keyframes qn-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
