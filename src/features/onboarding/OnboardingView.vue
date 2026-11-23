<script setup lang="ts">
import { QnBadge, QnBanner, QnButton, QnInput, QnPanelCard } from '@/components';
import { useOnboardingStore } from './store';

const store = useOnboardingStore();

async function handleSubmit(): Promise<void> {
  await store.submitRoot();
}
</script>

<template>
  <div class="qn-onboarding">
    <QnPanelCard
      class="qn-onboarding__card"
      title="欢迎使用 QuickNote"
      subtitle="首次启动请先指定笔记库根目录（纯本地 Markdown 文件夹）"
    >
      <template #header>
        <QnBadge tone="accent">首次初始化</QnBadge>
      </template>

      <form class="qn-onboarding__form" @submit.prevent="handleSubmit">
        <QnBanner v-if="store.errorMessage" tone="danger" title="无法初始化笔记库">
          {{ store.errorMessage }}
        </QnBanner>

        <QnInput
          id="qn-library-root-input"
          v-model="store.rootInput"
          label="笔记库目录路径"
          placeholder="~/Documents/QuickNote"
          hint="所有笔记均以标准 .md 文件形式保存在该目录，可随时迁移或备份。"
          :disabled="store.submitting"
          monospace
        />

        <label class="qn-onboarding__checkbox">
          <input v-model="store.createIfMissing" type="checkbox" :disabled="store.submitting" />
          <span>目录不存在时自动创建（包含默认 inbox.md）</span>
        </label>

        <div class="qn-onboarding__actions">
          <QnButton
            type="submit"
            variant="primary"
            :loading="store.submitting"
            :disabled="store.submitting"
          >
            确认并初始化工作台
          </QnButton>
        </div>
      </form>
    </QnPanelCard>
  </div>
</template>

<style scoped>
.qn-onboarding {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  padding: var(--qn-space-6);
  background-color: var(--qn-bg-sidebar);
}

.qn-onboarding__card {
  width: 100%;
  max-width: 520px;
}

.qn-onboarding__form {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-onboarding__checkbox {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
  cursor: pointer;
  user-select: none;
}

.qn-onboarding__actions {
  display: flex;
  justify-content: flex-end;
  padding-top: var(--qn-space-2);
}
</style>
