<script setup lang="ts">
export interface QnInputProps {
  modelValue: string;
  label?: string;
  placeholder?: string;
  hint?: string;
  error?: string | null;
  disabled?: boolean;
  readonly?: boolean;
  monospace?: boolean;
  id?: string;
}

withDefaults(defineProps<QnInputProps>(), {
  label: undefined,
  placeholder: '',
  hint: undefined,
  error: null,
  disabled: false,
  readonly: false,
  monospace: false,
  id: undefined,
});

const emit = defineEmits<{
  'update:modelValue': [value: string];
}>();

function onInput(event: Event): void {
  const target = event.target as HTMLInputElement | null;
  if (target) {
    emit('update:modelValue', target.value);
  }
}
</script>

<template>
  <div class="qn-input-field">
    <label v-if="label" :for="id" class="qn-input-field__label">
      {{ label }}
    </label>
    <input
      :id="id"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readonly"
      :class="[
        'qn-input-field__control',
        {
          'qn-input-field__control--mono': monospace,
          'qn-input-field__control--error': Boolean(error),
        },
      ]"
      @input="onInput"
    />
    <p v-if="error" class="qn-input-field__error">{{ error }}</p>
    <p v-else-if="hint" class="qn-input-field__hint">{{ hint }}</p>
  </div>
</template>

<style scoped>
.qn-input-field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-input-field__label {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-secondary);
}

.qn-input-field__control {
  width: 100%;
  height: 36px;
  padding: 0 var(--qn-space-3);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
  transition:
    border-color 0.14s ease,
    box-shadow 0.14s ease;
}

.qn-input-field__control--mono {
  font-family: var(--qn-font-mono);
}

.qn-input-field__control::placeholder {
  color: var(--qn-text-muted);
}

.qn-input-field__control:focus {
  outline: none;
  border-color: var(--qn-accent);
  box-shadow: 0 0 0 3px var(--qn-accent-soft);
}

.qn-input-field__control:disabled {
  background-color: var(--qn-bg-sidebar);
  color: var(--qn-text-muted);
  cursor: not-allowed;
}

.qn-input-field__control--error {
  border-color: var(--qn-danger);
}

.qn-input-field__hint {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-input-field__error {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}
</style>
