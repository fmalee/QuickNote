<script setup lang="ts">
import { ref } from 'vue';
import { QnBadge, QnButton, QnModalShell, QnPanelCard } from '@/components';
import { OnboardingView, useOnboardingStore } from '@/features/onboarding';
import { SettingsErrorBanner, VisualBaselinePreview, useSettingsStore } from '@/features/settings';

const onboardingStore = useOnboardingStore();
const settingsStore = useSettingsStore();

const baselinePreviewOpen = ref(false);
const activeNav = ref<'inbox' | 'projects' | 'tags' | 'recent' | 'workspaces'>('inbox');

const navItems: { id: 'inbox' | 'projects' | 'tags' | 'recent' | 'workspaces'; label: string }[] = [
  { id: 'inbox', label: '收件箱' },
  { id: 'projects', label: '项目' },
  { id: 'tags', label: '标签' },
  { id: 'recent', label: '最近' },
  { id: 'workspaces', label: '工作区' },
];
</script>

<template>
  <div class="qn-main-shell" data-testid="main-workbench-shell">
    <div v-if="settingsStore.isSettingsInvalid" class="qn-main-shell__top-banner">
      <SettingsErrorBanner :status="settingsStore.loadStatus" />
    </div>

    <div
      v-if="onboardingStore.isLocked"
      class="qn-main-shell__locked"
      data-testid="main-onboarding-lock"
    >
      <OnboardingView />
    </div>

    <div v-else class="qn-main-shell__workbench" data-testid="main-workbench-unlocked">
      <header class="qn-main-shell__titlebar">
        <div class="qn-main-shell__brand">
          <span class="qn-main-shell__title">QuickNote</span>
          <QnBadge tone="neutral">{{ onboardingStore.libraryInfo?.root }}</QnBadge>
        </div>
        <div class="qn-main-shell__titlebar-actions">
          <QnButton variant="ghost" size="sm" @click="baselinePreviewOpen = true">
            视觉基线预览
          </QnButton>
          <QnBadge tone="accent">⌃⌥Space 捕捉</QnBadge>
        </div>
      </header>

      <div class="qn-main-shell__body">
        <aside class="qn-main-shell__nav-col">
          <div class="qn-main-shell__nav-tabs">
            <button
              v-for="item in navItems"
              :key="item.id"
              type="button"
              :class="[
                'qn-main-shell__nav-btn',
                { 'qn-main-shell__nav-btn--active': activeNav === item.id },
              ]"
              @click="activeNav = item.id"
            >
              {{ item.label }}
            </button>
          </div>
          <div class="qn-main-shell__note-list">
            <QnPanelCard title="M0 工作台骨架" subtitle="笔记目录与列表加载将于 M1 接入">
              <p class="qn-main-shell__placeholder-text">
                当前库已就绪，可通过 <code>⌃⌥Space</code> 呼出 Quick Bar 或按
                <code>⌘,</code> 打开设置窗口。
              </p>
            </QnPanelCard>
          </div>
        </aside>

        <section class="qn-main-shell__editor-col">
          <div class="qn-main-shell__tabbar">
            <span class="qn-main-shell__tab qn-main-shell__tab--active">inbox.md</span>
          </div>
          <div class="qn-main-shell__workspace">
            <div class="qn-main-shell__editor-stage">
              <div class="qn-main-shell__editor-paper">
                <h1 class="qn-main-shell__doc-title">收件箱（M0 骨架视图）</h1>
                <p class="qn-main-shell__placeholder-text">
                  正文阅读宽度受 <code>--qn-editor-max-width</code>（900px）约束；CodeMirror 6
                  编辑器实例将于 M1 阶段挂载。
                </p>
              </div>
            </div>
            <aside class="qn-main-shell__right-panel">
              <div class="qn-main-shell__panel-header">大纲 / 右侧面板组</div>
              <p class="qn-main-shell__placeholder-text">M1 阶段接入大纲与关联面板。</p>
            </aside>
          </div>
        </section>
      </div>

      <footer class="qn-main-shell__statusbar">
        <span>库状态：已连接</span>
        <span>热键：{{ settingsStore.hotkeyStatus?.combo ?? 'ctrl+alt+space' }}</span>
      </footer>
    </div>

    <QnModalShell
      :open="baselinePreviewOpen"
      title="视觉基线预览"
      max-width="lg"
      @close="baselinePreviewOpen = false"
    >
      <div class="qn-main-shell__baseline-scroll">
        <VisualBaselinePreview />
      </div>
      <template #footer>
        <QnButton variant="primary" size="sm" @click="baselinePreviewOpen = false">关闭</QnButton>
      </template>
    </QnModalShell>
  </div>
</template>

<style scoped>
.qn-main-shell {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  background-color: var(--qn-bg-base);
}

.qn-main-shell__top-banner {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-main-shell__locked {
  flex: 1;
  min-height: 0;
}

.qn-main-shell__workbench {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.qn-main-shell__titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
  height: 44px;
  padding: 0 var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-main-shell__brand {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  min-width: 0;
}

.qn-main-shell__title {
  font-size: var(--qn-text-sm);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-main-shell__titlebar-actions {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-main-shell__body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.qn-main-shell__nav-col {
  width: 300px;
  flex-shrink: 0;
  border-right: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  display: flex;
  flex-direction: column;
  padding: var(--qn-space-3);
  gap: var(--qn-space-3);
}

.qn-main-shell__nav-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: var(--qn-space-1);
}

.qn-main-shell__nav-btn {
  padding: var(--qn-space-1) var(--qn-space-2);
  border: none;
  border-radius: var(--qn-radius-sm);
  background: transparent;
  color: var(--qn-text-secondary);
  font-size: var(--qn-text-xs);
  font-weight: 500;
  cursor: pointer;
}

.qn-main-shell__nav-btn:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-main-shell__nav-btn--active {
  background-color: var(--qn-bg-active);
  color: var(--qn-text-primary);
}

.qn-main-shell__note-list {
  flex: 1;
  overflow-y: auto;
}

.qn-main-shell__editor-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.qn-main-shell__tabbar {
  display: flex;
  align-items: center;
  height: 36px;
  padding: 0 var(--qn-space-3);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
}

.qn-main-shell__tab {
  padding: var(--qn-space-1) var(--qn-space-3);
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-main-shell__tab--active {
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  border: 1px solid var(--qn-border-subtle);
}

.qn-main-shell__workspace {
  flex: 1;
  min-height: 0;
  display: flex;
}

.qn-main-shell__editor-stage {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: var(--qn-space-6);
  display: flex;
  justify-content: center;
}

.qn-main-shell__editor-paper {
  width: 100%;
  max-width: var(--qn-editor-max-width);
}

.qn-main-shell__doc-title {
  margin: 0 0 var(--qn-space-3);
  font-size: var(--qn-text-lg);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-main-shell__right-panel {
  width: 260px;
  flex-shrink: 0;
  border-left: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  padding: var(--qn-space-3);
}

.qn-main-shell__panel-header {
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-secondary);
  margin-bottom: var(--qn-space-2);
}

.qn-main-shell__placeholder-text {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-main-shell__statusbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 26px;
  padding: 0 var(--qn-space-4);
  border-top: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-sidebar);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-main-shell__baseline-scroll {
  max-height: 70vh;
  overflow-y: auto;
}
</style>
