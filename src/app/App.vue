<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { useOnboardingStore } from '@/features/onboarding';
import { QuickBarView, useQuickBarStore } from '@/features/quickbar';
import { SettingsView, useSettingsStore } from '@/features/settings';
import { ToastView, useToastStore } from '@/features/toast';
import { resolveWindowKind, type WindowKind } from '@/lib/utils';
import MainWorkbenchShell from './MainWorkbenchShell.vue';

const windowKind = computed<WindowKind>(() => {
  if (typeof window === 'undefined') {
    return 'main';
  }
  return resolveWindowKind(window.location.search);
});

const onboardingStore = useOnboardingStore();
const settingsStore = useSettingsStore();
const quickBarStore = useQuickBarStore();
const toastStore = useToastStore();

onMounted(() => {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-window', windowKind.value);
  }

  void settingsStore.initialize().catch(() => {
    // 非 Tauri 浏览器纯预览模式允许静默回退默认主题
  });

  if (windowKind.value === 'main') {
    void onboardingStore.refreshLibraryInfo().catch(() => {
      // 非 Tauri 浏览器纯预览模式保持未配置引导态
    });
  } else if (windowKind.value === 'quickbar') {
    void quickBarStore.initialize().catch(() => {
      // 非 Tauri 浏览器纯预览模式保持 S1 引导态
    });
  } else if (windowKind.value === 'toast') {
    void toastStore.initialize().catch(() => {
      // 非 Tauri 浏览器纯预览模式等待事件
    });
  }
});

onUnmounted(() => {
  settingsStore.dispose();
  quickBarStore.dispose();
  toastStore.dispose();
});
</script>

<template>
  <QuickBarView v-if="windowKind === 'quickbar'" />
  <ToastView v-else-if="windowKind === 'toast'" />
  <SettingsView v-else-if="windowKind === 'settings'" />
  <MainWorkbenchShell v-else />
</template>
