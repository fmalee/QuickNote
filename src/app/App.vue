<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { QnBanner } from '@/components';
import { useOnboardingStore } from '@/features/onboarding';
import { QuickBarView, useQuickBarStore } from '@/features/quickbar';
import { SettingsView, useSettingsStore } from '@/features/settings';
import { ToastView, useToastStore } from '@/features/toast';
import { resolveWindowKind, type WindowKind } from '@/lib/utils';
import { formatStartupError, isTauriRuntime, useAppBootstrapStore } from './bootstrapStore';
import MainWorkbenchShell from './MainWorkbenchShell.vue';

const windowKind = computed<WindowKind>(() => {
  if (typeof window === 'undefined') {
    return 'main';
  }
  return resolveWindowKind(window.location.search);
});

const bootstrapStore = useAppBootstrapStore();
const onboardingStore = useOnboardingStore();
const settingsStore = useSettingsStore();
const quickBarStore = useQuickBarStore();
const toastStore = useToastStore();

async function initializeApp(): Promise<void> {
  bootstrapStore.setInitError(null);
  const inTauri = isTauriRuntime();
  const tasks: Promise<unknown>[] = [settingsStore.initialize()];

  if (windowKind.value === 'main') {
    tasks.push(onboardingStore.refreshLibraryInfo());
  } else if (windowKind.value === 'quickbar') {
    tasks.push(quickBarStore.initialize());
  } else if (windowKind.value === 'toast') {
    tasks.push(toastStore.initialize());
  }

  const results = await Promise.allSettled(tasks);
  if (!inTauri) {
    return;
  }

  const rejected = results.find((res): res is PromiseRejectedResult => res.status === 'rejected');
  if (rejected) {
    bootstrapStore.setInitError(formatStartupError(rejected.reason));
  }
}

onMounted(() => {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-window', windowKind.value);
  }
  void initializeApp();
});

onUnmounted(() => {
  settingsStore.dispose();
  quickBarStore.dispose();
  toastStore.dispose();
});
</script>

<template>
  <div
    v-if="bootstrapStore.initError"
    class="qn-app-init-error"
    data-testid="app-init-error-banner"
  >
    <QnBanner tone="danger" title="应用初始化失败">
      {{ bootstrapStore.initError }}
    </QnBanner>
  </div>
  <QuickBarView v-if="windowKind === 'quickbar'" />
  <ToastView v-else-if="windowKind === 'toast'" />
  <SettingsView v-else-if="windowKind === 'settings'" />
  <MainWorkbenchShell v-else />
</template>

<style scoped>
.qn-app-init-error {
  padding: var(--qn-space-2) var(--qn-space-4);
  background-color: var(--qn-bg-base);
}
</style>
