<script setup lang="ts">
import { computed } from 'vue';
import type { SettingsLoadStatus } from '@/bindings/types';
import { QnBadge, QnBanner } from '@/components';

const props = defineProps<{
  status: SettingsLoadStatus | null;
}>();

const locationLabel = computed<string>(() => {
  if (!props.status) {
    return '';
  }
  const parts: string[] = [];
  if (props.status.errorLine !== null) {
    parts.push(`第 ${props.status.errorLine} 行`);
  }
  if (props.status.errorColumn !== null) {
    parts.push(`第 ${props.status.errorColumn} 列`);
  }
  return parts.join(' ');
});
</script>

<template>
  <QnBanner
    v-if="status && !status.valid"
    tone="danger"
    title="settings.yaml 解析失败（已回退至安全配置，原文件未被覆盖）"
    data-testid="settings-error-banner"
  >
    <div class="qn-settings-error-detail">
      <QnBadge v-if="locationLabel" tone="danger">{{ locationLabel }}</QnBadge>
      <span class="qn-settings-error-msg">
        {{ status.errorMessage ?? '配置文件存在语法或字段错误，请检查 settings.yaml。' }}
      </span>
    </div>
  </QnBanner>
</template>

<style scoped>
.qn-settings-error-detail {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}

.qn-settings-error-msg {
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-primary);
}
</style>
