<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { QnBanner, QnButton, QnInput, QnModalShell } from '@/components';

const props = defineProps<{
  open: boolean;
  oldTag: string;
  existingTags: Set<string>;
  mutating: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'submit', oldTag: string, newTag: string): void;
}>();

const draft = ref<string>('');

watch(
  () => [props.open, props.oldTag] as const,
  ([isOpen, oldTag]) => {
    if (isOpen) {
      draft.value = oldTag;
    }
  },
  { immediate: true },
);

const normalizedDraft = computed<string>(() => draft.value.trim().replace(/^#+/, ''));

const isMergeTarget = computed<boolean>(() => {
  const target = normalizedDraft.value;
  return Boolean(target && target !== props.oldTag && props.existingTags.has(target));
});

const canSubmit = computed<boolean>(() => {
  const target = normalizedDraft.value;
  return Boolean(target && target !== props.oldTag && !props.mutating);
});

function handleSubmit(): void {
  if (!canSubmit.value) {
    return;
  }
  emit('submit', props.oldTag, normalizedDraft.value);
}
</script>

<template>
  <QnModalShell
    :open="props.open"
    title="重命名 / 合并标签"
    max-width="sm"
    data-testid="tag-rename-modal"
    @close="emit('close')"
  >
    <div class="qn-tag-rename-modal__body">
      <div class="qn-tag-rename-modal__row">
        <span class="qn-tag-rename-modal__label">原标签</span>
        <code class="qn-tag-rename-modal__old">#{{ props.oldTag }}</code>
      </div>
      <div class="qn-tag-rename-modal__row">
        <label class="qn-tag-rename-modal__label" for="qn-tag-rename-input">新标签</label>
        <QnInput
          id="qn-tag-rename-input"
          v-model="draft"
          placeholder="输入新标签名（不含 #）"
          data-testid="tag-rename-input"
          @keydown.enter.prevent="handleSubmit"
        />
      </div>
      <QnBanner v-if="isMergeTarget" tone="warning" data-testid="tag-merge-warning">
        目标标签
        <code>#{{ normalizedDraft }}</code>
        已存在，确认后将把原标签下的全部笔记<strong>合并</strong>到该标签下（自动去重，不覆盖注释与未知字段）。
      </QnBanner>
    </div>
    <template #footer>
      <QnButton size="sm" variant="ghost" @click="emit('close')">取消</QnButton>
      <QnButton
        size="sm"
        variant="primary"
        :disabled="!canSubmit"
        data-testid="tag-rename-submit-btn"
        @click="handleSubmit"
      >
        {{ isMergeTarget ? '确认合并' : '确认重命名' }}
      </QnButton>
    </template>
  </QnModalShell>
</template>

<style scoped>
.qn-tag-rename-modal__body {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-tag-rename-modal__row {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-tag-rename-modal__label {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-tag-rename-modal__old {
  font-family: var(--qn-font-mono);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
}
</style>
