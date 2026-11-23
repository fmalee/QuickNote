import { ref } from 'vue';
import { defineStore } from 'pinia';
import { isAppError } from '@/lib/ipc/commands';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export function formatStartupError(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message}（${error.detail}）` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '应用初始化失败，请检查配置或重启重试。';
}

export const useAppBootstrapStore = defineStore('app-bootstrap', () => {
  const initError = ref<string | null>(null);

  function setInitError(message: string | null): void {
    initError.value = message;
  }

  return {
    initError,
    setInitError,
  };
});
