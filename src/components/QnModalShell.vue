<script setup lang="ts">
export interface QnModalShellProps {
  open: boolean;
  title?: string;
  maxWidth?: 'sm' | 'md' | 'lg';
}

withDefaults(defineProps<QnModalShellProps>(), {
  title: undefined,
  maxWidth: 'md',
});

const emit = defineEmits<{
  close: [];
}>();
</script>

<template>
  <div v-if="open" class="qn-modal-shell" role="dialog" aria-modal="true">
    <div class="qn-modal-shell__backdrop" @click="emit('close')" />
    <div :class="['qn-modal-shell__panel', `qn-modal-shell__panel--${maxWidth}`]">
      <header v-if="title || $slots.header" class="qn-modal-shell__header">
        <slot name="header">
          <h2 class="qn-modal-shell__title">{{ title }}</h2>
        </slot>
      </header>
      <div class="qn-modal-shell__body">
        <slot />
      </div>
      <footer v-if="$slots.footer" class="qn-modal-shell__footer">
        <slot name="footer" />
      </footer>
    </div>
  </div>
</template>

<style scoped>
.qn-modal-shell {
  position: fixed;
  inset: 0;
  z-index: var(--qn-z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--qn-space-6);
}

.qn-modal-shell__backdrop {
  position: absolute;
  inset: 0;
  background-color: var(--qn-bg-backdrop);
}

.qn-modal-shell__panel {
  position: relative;
  width: 100%;
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-lg);
  box-shadow: var(--qn-shadow-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.qn-modal-shell__panel--sm {
  max-width: 420px;
}

.qn-modal-shell__panel--md {
  max-width: 540px;
}

.qn-modal-shell__panel--lg {
  max-width: 720px;
}

.qn-modal-shell__header {
  padding: var(--qn-space-4) var(--qn-space-5);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-modal-shell__title {
  margin: 0;
  font-size: var(--qn-text-md);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-modal-shell__body {
  padding: var(--qn-space-5);
}

.qn-modal-shell__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--qn-space-2);
  padding: var(--qn-space-3) var(--qn-space-5);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}
</style>
