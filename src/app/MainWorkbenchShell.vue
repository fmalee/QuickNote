<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { QnBadge, QnBanner, QnButton, QnModalShell } from '@/components';
import { EditorTabBar, InboxBloatBanner, useEditorTabsStore } from '@/features/editor';
import { NotesSidebar, useNotesStore } from '@/features/notes';
import { OnboardingView, useOnboardingStore } from '@/features/onboarding';
import { SettingsErrorBanner, VisualBaselinePreview, useSettingsStore } from '@/features/settings';

const onboardingStore = useOnboardingStore();
const settingsStore = useSettingsStore();
const notesStore = useNotesStore();
const editorTabsStore = useEditorTabsStore();

const baselinePreviewOpen = ref(false);

watch(
  () => onboardingStore.libraryInfo?.root ?? null,
  (nextRoot) => {
    if (nextRoot) {
      void Promise.all([notesStore.initialize(), editorTabsStore.restoreTabsFromUiState()]);
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  notesStore.dispose();
  editorTabsStore.dispose();
});
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
        <NotesSidebar />

        <section class="qn-main-shell__editor-col">
          <EditorTabBar />
          <InboxBloatBanner />

          <div class="qn-main-shell__workspace">
            <div class="qn-main-shell__editor-stage">
              <div
                v-if="editorTabsStore.activeTab?.isMissing"
                class="qn-main-shell__editor-paper"
                data-testid="editor-missing-doc"
              >
                <QnBanner tone="danger" title="文件已移走或失联">
                  未在当前笔记库或磁盘路径中找到：{{ editorTabsStore.activeTab.relPath }}
                  <template #actions>
                    <QnButton
                      size="sm"
                      variant="secondary"
                      @click="editorTabsStore.closeTab(editorTabsStore.activeTab.key)"
                    >
                      关闭标签
                    </QnButton>
                  </template>
                </QnBanner>
              </div>

              <div
                v-else-if="editorTabsStore.activeDoc"
                class="qn-main-shell__editor-paper"
                data-testid="editor-doc-stage"
              >
                <header class="qn-main-shell__doc-header">
                  <h1 class="qn-main-shell__doc-title" data-testid="editor-doc-title">
                    {{ editorTabsStore.activeDoc.meta.title || editorTabsStore.activeDoc.relPath }}
                  </h1>
                  <div class="qn-main-shell__doc-meta">
                    <QnBadge v-if="editorTabsStore.activeDoc.meta.docType" tone="neutral">
                      {{ editorTabsStore.activeDoc.meta.docType }}
                    </QnBadge>
                    <QnBadge
                      v-for="tag in editorTabsStore.activeDoc.meta.tags"
                      :key="tag"
                      tone="accent"
                    >
                      #{{ tag }}
                    </QnBadge>
                    <QnBadge
                      v-if="editorTabsStore.activeDoc.meta.metaError"
                      tone="warning"
                      data-testid="editor-doc-meta-error"
                    >
                      Frontmatter 异常：{{ editorTabsStore.activeDoc.meta.metaError }}
                    </QnBadge>
                  </div>
                </header>

                <div class="qn-main-shell__body-stage" data-testid="editor-body-stage">
                  <pre class="qn-main-shell__body-preview">{{
                    editorTabsStore.activeDoc.body
                  }}</pre>
                </div>
              </div>

              <div v-else class="qn-main-shell__editor-paper">
                <p class="qn-main-shell__placeholder-text">从左侧选择或新建一篇笔记。</p>
              </div>
            </div>
            <aside class="qn-main-shell__right-panel">
              <div class="qn-main-shell__panel-header">大纲 / 右侧面板组</div>
              <p class="qn-main-shell__placeholder-text">M1-04b 阶段接入大纲与关联面板。</p>
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

.qn-main-shell__editor-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.qn-main-shell__editor-banner {
  padding: var(--qn-space-2) var(--qn-space-4);
  border-bottom: 1px solid var(--qn-border-subtle);
  background-color: var(--qn-bg-base);
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

.qn-main-shell__doc-header {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
  margin-bottom: var(--qn-space-4);
  padding-bottom: var(--qn-space-3);
  border-bottom: 1px solid var(--qn-border-subtle);
}

.qn-main-shell__doc-title {
  margin: 0;
  font-size: var(--qn-text-lg);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-main-shell__doc-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--qn-space-1);
}

.qn-main-shell__body-stage {
  min-height: 240px;
}

.qn-main-shell__body-preview {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--qn-font-sans);
  font-size: var(--qn-text-base);
  line-height: 1.7;
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
