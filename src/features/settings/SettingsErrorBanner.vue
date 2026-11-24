<script setup lang="ts">
import { computed, ref } from 'vue';
import type { SettingsLoadStatus } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton } from '@/components';
import {
  extractSettingsErrorMessage,
  openSettingsYamlForRepair,
  revealSettingsYamlInFinder,
} from './api';

const props = defineProps<{
  status: SettingsLoadStatus | null;
}>();

const actionPending = ref<'open' | 'reveal' | null>(null);
const actionError = ref<string | null>(null);

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

async function handleOpenForRepair(): Promise<void> {
  if (actionPending.value) {
    return;
  }
  actionPending.value = 'open';
  actionError.value = null;
  try {
    await openSettingsYamlForRepair();
  } catch (error) {
    actionError.value = extractSettingsErrorMessage(error);
  } finally {
    actionPending.value = null;
  }
}

async function handleRevealInFinder(): Promise<void> {
  if (actionPending.value) {
    return;
  }
  actionPending.value = 'reveal';
  actionError.value = null;
  try {
    await revealSettingsYamlInFinder();
  } catch (error) {
    actionError.value = extractSettingsErrorMessage(error);
  } finally {
    actionPending.value = null;
  }
}
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
    <p
      v-if="actionError"
      class="qn-settings-error-action-err"
      role="alert"
      data-testid="settings-error-banner-action-error"
    >
      {{ actionError }}
    </p>
    <template #actions>
      <div class="qn-settings-error-actions">
        <QnButton
          variant="primary"
          size="sm"
          :loading="actionPending === 'open'"
          :disabled="actionPending !== null"
          data-testid="open-settings-yaml-btn"
          @click="handleOpenForRepair"
        >
          打开 settings.yaml 修复
        </QnButton>
        <QnButton
          variant="secondary"
          size="sm"
          :loading="actionPending === 'reveal'"
          :disabled="actionPending !== null"
          data-testid="reveal-settings-yaml-btn"
          @click="handleRevealInFinder"
        >
          在 Finder 中显示
        </QnButton>
      </div>
    </template>
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

.qn-settings-error-action-err {
  margin: var(--qn-space-1) 0 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}

.qn-settings-error-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}
</style>
