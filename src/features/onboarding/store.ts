import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import type { LibraryInfo } from '@/bindings/types';
import {
  configureLibraryRoot,
  extractOnboardingErrorMessage,
  fetchLibraryInfo,
  openMainWindow,
} from './api';

export const DEFAULT_LIBRARY_ROOT = '~/Documents/QuickNote';

export const useOnboardingStore = defineStore('onboarding', () => {
  const libraryInfo = ref<LibraryInfo | null>(null);
  const rootInput = ref<string>(DEFAULT_LIBRARY_ROOT);
  const createIfMissing = ref<boolean>(true);
  const loading = ref<boolean>(false);
  const submitting = ref<boolean>(false);
  const errorMessage = ref<string | null>(null);

  const isConfigured = computed<boolean>(() => Boolean(libraryInfo.value?.root));
  const isLocked = computed<boolean>(() => !isConfigured.value);

  async function refreshLibraryInfo(): Promise<LibraryInfo | null> {
    loading.value = true;
    errorMessage.value = null;
    try {
      const info = await fetchLibraryInfo();
      libraryInfo.value = info;
      if (info.root) {
        rootInput.value = info.root;
      }
      return info;
    } catch (error) {
      errorMessage.value = extractOnboardingErrorMessage(error);
      return null;
    } finally {
      loading.value = false;
    }
  }

  async function submitRoot(
    customRoot?: string,
    customCreateIfMissing?: boolean,
  ): Promise<boolean> {
    const targetRoot = (customRoot ?? rootInput.value).trim();
    if (!targetRoot) {
      errorMessage.value = '请输入笔记库目录路径。';
      return false;
    }

    submitting.value = true;
    errorMessage.value = null;
    try {
      const info = await configureLibraryRoot({
        root: targetRoot,
        createIfMissing: customCreateIfMissing ?? createIfMissing.value,
      });
      libraryInfo.value = info;
      if (info.root) {
        rootInput.value = info.root;
      }
      return Boolean(info.root);
    } catch (error) {
      errorMessage.value = extractOnboardingErrorMessage(error);
      return false;
    } finally {
      submitting.value = false;
    }
  }

  async function showMainForOnboarding(): Promise<void> {
    await openMainWindow();
  }

  return {
    libraryInfo,
    rootInput,
    createIfMissing,
    loading,
    submitting,
    errorMessage,
    isConfigured,
    isLocked,
    refreshLibraryInfo,
    submitRoot,
    showMainForOnboarding,
  };
});
