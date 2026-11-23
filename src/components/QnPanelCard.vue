<script setup lang="ts">
export interface QnPanelCardProps {
  title?: string;
  subtitle?: string;
  padded?: boolean;
}

withDefaults(defineProps<QnPanelCardProps>(), {
  title: undefined,
  subtitle: undefined,
  padded: true,
});
</script>

<template>
  <section class="qn-panel-card">
    <header v-if="title || subtitle || $slots.header" class="qn-panel-card__header">
      <div class="qn-panel-card__titles">
        <h3 v-if="title" class="qn-panel-card__title">{{ title }}</h3>
        <p v-if="subtitle" class="qn-panel-card__subtitle">{{ subtitle }}</p>
      </div>
      <div v-if="$slots.header" class="qn-panel-card__extra">
        <slot name="header" />
      </div>
    </header>
    <div :class="['qn-panel-card__body', { 'qn-panel-card__body--padded': padded }]">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.qn-panel-card {
  background-color: var(--qn-bg-elevated);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-md);
  box-shadow: var(--qn-shadow-sm);
  overflow: hidden;
}

.qn-panel-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  padding: var(--qn-space-3) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-panel-card__titles {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.qn-panel-card__title {
  margin: 0;
  font-size: var(--qn-text-base);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-panel-card__subtitle {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-panel-card__extra {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  flex-shrink: 0;
}

.qn-panel-card__body--padded {
  padding: var(--qn-space-4);
}
</style>
