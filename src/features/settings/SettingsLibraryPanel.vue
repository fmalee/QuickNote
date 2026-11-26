<script setup lang="ts">
import { onMounted, reactive, ref, watch } from 'vue';
import { QnBadge, QnButton, QnInput, QnPanelCard } from '@/components';
import { indexRebuild, notesBackfillMissingIds } from '@/lib/ipc/commands';
import { extractSettingsErrorMessage } from './api';
import { useSettingsStore } from './store';

const MIN_ACTIVE_EDITOR_RENDERS = 2;
const MAX_ACTIVE_EDITOR_RENDERS = 20;
const DEFAULT_ACTIVE_EDITOR_RENDERS = 5;

const store = useSettingsStore();

const libraryRootInput = ref('');
const createIfMissing = ref(true);
const newTemplateName = ref('');
const maxActiveEditorRendersInput = ref(DEFAULT_ACTIVE_EDITOR_RENDERS);
const rebuildingIndex = ref(false);
const rebuildIndexFeedback = ref<string | null>(null);
const rebuildIndexError = ref<string | null>(null);
const backfillingIds = ref(false);
const backfillIdsFeedback = ref<string | null>(null);
const backfillIdsError = ref<string | null>(null);

const inboxForm = reactive({
  bloatThresholdLines: 500,
  autoCreateMissingSection: false,
  lastActiveTtlHours: 24,
  destroyAfterHiddenMinutes: 10,
});

watch(
  () => store.snapshot?.settings,
  (settings) => {
    if (!settings) {
      return;
    }
    libraryRootInput.value = settings.libraryRoot ?? '';
    inboxForm.bloatThresholdLines = settings.inbox.bloatThresholdLines;
    inboxForm.autoCreateMissingSection = settings.quickBar.autoCreateMissingSection;
    inboxForm.lastActiveTtlHours = settings.quickBar.lastActiveTtlHours;
    inboxForm.destroyAfterHiddenMinutes = settings.quickBar.destroyAfterHiddenMinutes;
    maxActiveEditorRendersInput.value =
      settings.maxActiveEditorRenders ?? DEFAULT_ACTIVE_EDITOR_RENDERS;
  },
  { immediate: true, deep: true },
);

onMounted(() => {
  void store.loadTemplates();
});

async function applyLibraryRoot(): Promise<void> {
  await store.updateLibraryRoot(libraryRootInput.value, createIfMissing.value);
}

async function commitMaxActiveEditorRenders(): Promise<void> {
  const raw = Number(maxActiveEditorRendersInput.value);
  const clamped = Number.isFinite(raw)
    ? Math.min(MAX_ACTIVE_EDITOR_RENDERS, Math.max(MIN_ACTIVE_EDITOR_RENDERS, Math.round(raw)))
    : DEFAULT_ACTIVE_EDITOR_RENDERS;
  maxActiveEditorRendersInput.value = clamped;
  await store.patchSettings({
    maxActiveEditorRenders: clamped,
  });
}

async function handleRebuildIndex(): Promise<void> {
  if (rebuildingIndex.value || backfillingIds.value) {
    return;
  }
  rebuildingIndex.value = true;
  rebuildIndexFeedback.value = null;
  rebuildIndexError.value = null;
  try {
    const status = await indexRebuild();
    rebuildIndexFeedback.value = `缓存索引已重建完成，共索引 ${status.notesCount} 篇笔记。`;
  } catch (error) {
    rebuildIndexError.value = extractSettingsErrorMessage(error);
  } finally {
    rebuildingIndex.value = false;
  }
}

async function handleBackfillMissingIds(): Promise<void> {
  if (backfillingIds.value || rebuildingIndex.value) {
    return;
  }
  backfillingIds.value = true;
  backfillIdsFeedback.value = null;
  backfillIdsError.value = null;
  try {
    const result = await notesBackfillMissingIds();
    const failedSuffix = result.failed > 0 ? `，失败 ${result.failed} 篇` : '';
    backfillIdsFeedback.value = `扫描 ${result.scanned} 篇：已补全 ${result.updated} 篇，已有 ID 跳过 ${result.skippedExisting} 篇，YAML 异常跳过 ${result.skippedBrokenYaml} 篇${failedSuffix}。`;
  } catch (error) {
    backfillIdsError.value = extractSettingsErrorMessage(error);
  } finally {
    backfillingIds.value = false;
  }
}

async function commitInboxBloatThreshold(): Promise<void> {
  const val = Math.max(
    10,
    Math.round(
      Number.isFinite(inboxForm.bloatThresholdLines) ? inboxForm.bloatThresholdLines : 500,
    ),
  );
  inboxForm.bloatThresholdLines = val;
  await store.patchSettings({
    inbox: { bloatThresholdLines: val },
  });
}

async function onToggleAutoCreateSection(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement | null;
  if (!target) {
    return;
  }
  inboxForm.autoCreateMissingSection = target.checked;
  await store.patchSettings({
    quickBar: { autoCreateMissingSection: inboxForm.autoCreateMissingSection },
  });
}

async function commitQuickBarTiming(): Promise<void> {
  const ttl = Math.max(
    1,
    Math.round(Number.isFinite(inboxForm.lastActiveTtlHours) ? inboxForm.lastActiveTtlHours : 24),
  );
  const destroyMins = Math.max(
    1,
    Math.round(
      Number.isFinite(inboxForm.destroyAfterHiddenMinutes)
        ? inboxForm.destroyAfterHiddenMinutes
        : 10,
    ),
  );
  inboxForm.lastActiveTtlHours = ttl;
  inboxForm.destroyAfterHiddenMinutes = destroyMins;
  await store.patchSettings({
    quickBar: {
      lastActiveTtlHours: ttl,
      destroyAfterHiddenMinutes: destroyMins,
    },
  });
}

async function handleCreateTemplate(): Promise<void> {
  const created = await store.addTemplate(newTemplateName.value);
  if (created) {
    newTemplateName.value = '';
  }
}
</script>

<template>
  <div class="qn-library-panel">
    <QnPanelCard title="笔记库根目录" subtitle="切换或初始化本地 Markdown 笔记库路径">
      <div class="qn-library-panel__stack">
        <QnInput
          v-model="libraryRootInput"
          label="库根目录路径"
          placeholder="/Users/name/Documents/QuickNote"
          :disabled="store.isSettingsInvalid"
          monospace
          data-testid="library-root-input"
        />
        <div class="qn-library-panel__row">
          <label class="qn-library-panel__checkbox">
            <input v-model="createIfMissing" type="checkbox" :disabled="store.isSettingsInvalid" />
            <span>目录不存在时自动创建</span>
          </label>
          <QnButton
            variant="primary"
            size="sm"
            :loading="store.saving"
            :disabled="store.isSettingsInvalid"
            data-testid="apply-library-root-btn"
            @click="applyLibraryRoot"
          >
            应用库路径
          </QnButton>
        </div>
      </div>
    </QnPanelCard>

    <QnPanelCard
      title="常驻内存优化与缓存索引"
      subtitle="控制多标签编辑器常驻渲染上限（同步至 EditorStatePool），并支持一键清空重建本地缓存索引"
      data-testid="library-memory-and-cache-card"
    >
      <div class="qn-library-panel__stack">
        <div class="qn-library-panel__grid">
          <label class="qn-library-panel__field">
            <span class="qn-library-panel__label">
              常驻内存优化：最大同时保留渲染标签数（2–20，默认 5）
            </span>
            <input
              v-model.number="maxActiveEditorRendersInput"
              type="number"
              min="2"
              max="20"
              step="1"
              :disabled="store.isSettingsInvalid"
              data-testid="max-active-editor-renders-input"
              class="qn-library-panel__number"
              @change="commitMaxActiveEditorRenders"
            />
          </label>

          <div class="qn-library-panel__cache-action">
            <span class="qn-library-panel__label">本地 SQLite 派生缓存索引与笔记 ULID</span>
            <div class="qn-library-panel__action-group">
              <QnButton
                variant="secondary"
                size="sm"
                :loading="rebuildingIndex"
                :disabled="rebuildingIndex || backfillingIds"
                data-testid="rebuild-index-btn"
                @click="handleRebuildIndex"
              >
                一键清空重建缓存索引
              </QnButton>
              <QnButton
                variant="secondary"
                size="sm"
                :loading="backfillingIds"
                :disabled="backfillingIds || rebuildingIndex"
                data-testid="backfill-ids-btn"
                @click="handleBackfillMissingIds"
              >
                为缺失 ID 的笔记补全 ULID
              </QnButton>
            </div>
          </div>
        </div>

        <p
          v-if="rebuildIndexFeedback"
          class="qn-library-panel__feedback"
          data-testid="rebuild-index-feedback"
        >
          {{ rebuildIndexFeedback }}
        </p>
        <p
          v-if="rebuildIndexError"
          class="qn-library-panel__error"
          role="alert"
          data-testid="rebuild-index-error"
        >
          {{ rebuildIndexError }}
        </p>
        <p
          v-if="backfillIdsFeedback"
          class="qn-library-panel__feedback"
          data-testid="backfill-ids-feedback"
        >
          {{ backfillIdsFeedback }}
        </p>
        <p
          v-if="backfillIdsError"
          class="qn-library-panel__error"
          role="alert"
          data-testid="backfill-ids-error"
        >
          {{ backfillIdsError }}
        </p>
      </div>
    </QnPanelCard>

    <QnPanelCard title="收件箱与 Quick Bar 策略" subtitle="控制膨胀提醒与速记目标定位行为">
      <div class="qn-library-panel__grid">
        <label class="qn-library-panel__field">
          <span class="qn-library-panel__label">收件箱膨胀提醒阈值（行，≥ 10）</span>
          <input
            v-model.number="inboxForm.bloatThresholdLines"
            type="number"
            min="10"
            step="10"
            :disabled="store.isSettingsInvalid"
            data-testid="inbox-bloat-threshold-input"
            class="qn-library-panel__number"
            @change="commitInboxBloatThreshold"
          />
        </label>

        <label class="qn-library-panel__field">
          <span class="qn-library-panel__label">最近活跃文档有效期（小时）</span>
          <input
            v-model.number="inboxForm.lastActiveTtlHours"
            type="number"
            min="1"
            step="1"
            :disabled="store.isSettingsInvalid"
            data-testid="quickbar-ttl-hours-input"
            class="qn-library-panel__number"
            @change="commitQuickBarTiming"
          />
        </label>

        <label class="qn-library-panel__field">
          <span class="qn-library-panel__label">Quick Bar 隐藏后回收延迟（分钟）</span>
          <input
            v-model.number="inboxForm.destroyAfterHiddenMinutes"
            type="number"
            min="1"
            step="1"
            :disabled="store.isSettingsInvalid"
            data-testid="quickbar-destroy-minutes-input"
            class="qn-library-panel__number"
            @change="commitQuickBarTiming"
          />
        </label>

        <label class="qn-library-panel__checkbox qn-library-panel__checkbox--full">
          <input
            :checked="inboxForm.autoCreateMissingSection"
            type="checkbox"
            :disabled="store.isSettingsInvalid"
            data-testid="quickbar-auto-create-section-checkbox"
            @change="onToggleAutoCreateSection"
          />
          <span>Quick Bar 追加时自动创建缺失的目标分区（如 ## Inbox）</span>
        </label>
      </div>
    </QnPanelCard>

    <QnPanelCard title="模板管理 (_templates/)" subtitle="查看或新建自定义笔记模板">
      <div class="qn-library-panel__stack">
        <div class="qn-library-panel__template-create">
          <QnInput
            v-model="newTemplateName"
            placeholder="输入新模板名称（如 meeting-notes）"
            :disabled="store.isSettingsInvalid"
            data-testid="new-template-input"
          />
          <QnButton
            variant="secondary"
            size="sm"
            :loading="store.saving"
            :disabled="store.isSettingsInvalid || !newTemplateName.trim()"
            data-testid="create-template-btn"
            @click="handleCreateTemplate"
          >
            新建自定义模板
          </QnButton>
        </div>

        <div v-if="store.loadingTemplates" class="qn-library-panel__muted">正在加载模板列表…</div>
        <ul v-else-if="store.templates.length > 0" class="qn-library-panel__template-list">
          <li
            v-for="tpl in store.templates"
            :key="tpl.absPath"
            class="qn-library-panel__template-item"
          >
            <span class="qn-library-panel__template-title">{{ tpl.title }}</span>
            <QnBadge tone="neutral">{{ tpl.relPath ?? tpl.absPath }}</QnBadge>
          </li>
        </ul>
        <div v-else class="qn-library-panel__muted">暂无可用模板</div>
      </div>
    </QnPanelCard>
  </div>
</template>

<style scoped>
.qn-library-panel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-library-panel__stack {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-3);
}

.qn-library-panel__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--qn-space-3);
}

.qn-library-panel__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--qn-space-3);
}

.qn-library-panel__field {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
}

.qn-library-panel__cache-action {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--qn-space-1);
}

.qn-library-panel__action-group {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-library-panel__label {
  font-size: var(--qn-text-xs);
  font-weight: 500;
  color: var(--qn-text-secondary);
}

.qn-library-panel__number {
  height: 34px;
  padding: 0 var(--qn-space-2);
  border: 1px solid var(--qn-border-strong);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  color: var(--qn-text-primary);
  font-size: var(--qn-text-sm);
}

.qn-library-panel__number:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.qn-library-panel__checkbox {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
  font-size: var(--qn-text-sm);
  color: var(--qn-text-primary);
}

.qn-library-panel__checkbox--full {
  grid-column: 1 / -1;
  padding-top: var(--qn-space-1);
}

.qn-library-panel__template-create {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: var(--qn-space-2);
  align-items: start;
}

.qn-library-panel__template-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-library-panel__template-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--qn-space-2) var(--qn-space-3);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-library-panel__template-title {
  font-size: var(--qn-text-sm);
  font-weight: 500;
  color: var(--qn-text-primary);
}

.qn-library-panel__muted {
  font-size: var(--qn-text-sm);
  color: var(--qn-text-muted);
}

.qn-library-panel__feedback {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
}

.qn-library-panel__error {
  margin: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-danger);
}
</style>
