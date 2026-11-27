<script setup lang="ts">
import { ref } from 'vue';
import type { EditorTheme } from '@/bindings/types';
import { QnBadge, QnBanner, QnButton, QnInput, QnModalShell, QnPanelCard } from '@/components';
import { useSettingsStore } from './store';

const store = useSettingsStore();

const sampleInput = ref('~/Documents/QuickNote');
const sampleErrorInput = ref('invalid::yaml');
const modalOpen = ref(false);

const themeOptions: { value: EditorTheme; label: string }[] = [
  { value: 'light', label: '浅色主题' },
  { value: 'dark', label: '深色主题' },
  { value: 'system', label: '跟随系统' },
];

const colorTokens = [
  { token: '--qn-bg-base', label: 'Base 背景' },
  { token: '--qn-bg-elevated', label: 'Elevated 浮层' },
  { token: '--qn-bg-sidebar', label: 'Sidebar 侧栏' },
  { token: '--qn-bg-hover', label: 'Hover 悬停' },
  { token: '--qn-bg-active', label: 'Active 激活' },
  { token: '--qn-accent', label: 'Accent 主色' },
  { token: '--qn-success', label: 'Success 成功' },
  { token: '--qn-warning', label: 'Warning 警告' },
  { token: '--qn-danger', label: 'Danger 危险' },
];
</script>

<template>
  <div class="qn-baseline">
    <div class="qn-baseline__topbar">
      <div>
        <h2 class="qn-baseline__heading">视觉基线预览页（--qn-* Token）</h2>
        <p class="qn-baseline__subheading">
          用于 M0 阶段集中点验浅色/深色双主题对比度、排版密度、圆角与基础原子组件规范。
        </p>
      </div>
      <div class="qn-baseline__theme-switch">
        <QnButton
          v-for="opt in themeOptions"
          :key="opt.value"
          :variant="store.themePreference === opt.value ? 'primary' : 'secondary'"
          size="sm"
          :disabled="store.isSettingsInvalid"
          @click="store.setTheme(opt.value, { persist: true })"
        >
          {{ opt.label }}
        </QnButton>
      </div>
    </div>

    <QnPanelCard title="色彩 Token（语义映射）" subtitle="全部颜色均由 src/styles/tokens.css 驱动">
      <div class="qn-baseline__swatches">
        <div v-for="item in colorTokens" :key="item.token" class="qn-baseline__swatch">
          <span
            class="qn-baseline__swatch-dot"
            :style="{ backgroundColor: `var(${item.token})` }"
          />
          <div class="qn-baseline__swatch-meta">
            <span class="qn-baseline__swatch-label">{{ item.label }}</span>
            <code class="qn-baseline__swatch-token">{{ item.token }}</code>
          </div>
        </div>
      </div>
    </QnPanelCard>

    <QnPanelCard title="排版字阶与阅读宽度" subtitle="Sans / Mono 字体与 --qn-text-* 字号">
      <div class="qn-baseline__type-scale">
        <div class="qn-baseline__type-lg">Title LG（20px）— QuickNote 本地优先双链与捕捉工作台</div>
        <div class="qn-baseline__type-md">Heading MD（16px）— 快速记录 · 精确分区追加</div>
        <div class="qn-baseline__type-base">
          Body Base（14px）— 正文阅读最大宽度绑定 --qn-editor-max-width（900px）
        </div>
        <div class="qn-baseline__type-sm">Caption SM（12.5px）— 辅助说明、表单标签与列表摘要</div>
        <div class="qn-baseline__type-xs">Meta XS（11px）— 时间戳、快捷键提示与状态角标</div>
        <code class="qn-baseline__mono"
          >Mono: ~/Documents/QuickNote/inbox.md · ⌃⌥Space · ⌃⌥⌘Space</code
        >
      </div>
    </QnPanelCard>

    <QnPanelCard title="基础原子组件矩阵" subtitle="Button · Input · Badge · Banner · ModalShell">
      <div class="qn-baseline__components">
        <div class="qn-baseline__row">
          <QnButton variant="primary">Primary 主按钮</QnButton>
          <QnButton variant="secondary">Secondary 次按钮</QnButton>
          <QnButton variant="ghost">Ghost 幽灵</QnButton>
          <QnButton variant="danger">Danger 危险</QnButton>
          <QnButton variant="primary" size="sm">Small 小号</QnButton>
          <QnButton variant="primary" :loading="true">加载中</QnButton>
        </div>

        <div class="qn-baseline__row">
          <QnBadge tone="neutral">Neutral</QnBadge>
          <QnBadge tone="accent">Accent</QnBadge>
          <QnBadge tone="success">Success · OK</QnBadge>
          <QnBadge tone="warning">Warning · S1</QnBadge>
          <QnBadge tone="danger">Danger · Conflict</QnBadge>
        </div>

        <div class="qn-baseline__inputs">
          <QnInput
            v-model="sampleInput"
            label="标准路径输入（等宽）"
            hint="支持绝对路径或 ~ 开头目录"
            monospace
          />
          <QnInput
            v-model="sampleErrorInput"
            label="校验错误状态"
            error="第 14 行 第 3 列：未知配置字段"
            monospace
          />
        </div>

        <div class="qn-baseline__banners">
          <QnBanner tone="info" title="Info 提示横幅">当前处于 M0 视觉基线预览模式。</QnBanner>
          <QnBanner tone="warning" title="Warning 警告横幅">全局热键可能被系统占用。</QnBanner>
          <QnBanner tone="danger" title="Danger 错误横幅">
            settings.yaml 第 14 行 第 3 列存在语法错误，已回退安全配置。
          </QnBanner>
          <QnBanner tone="success" title="Success 成功横幅">
            笔记库已初始化完成，Quick Bar 与主工作台同步可用。
          </QnBanner>
        </div>

        <div class="qn-baseline__row">
          <QnButton variant="secondary" @click="modalOpen = true">预览 ModalShell 弹窗</QnButton>
        </div>
      </div>
    </QnPanelCard>

    <QnModalShell :open="modalOpen" title="ModalShell 弹窗基线" @close="modalOpen = false">
      <p class="qn-baseline__modal-text">
        该弹窗使用 <code>--qn-z-modal</code> 层级与
        <code>--qn-radius-lg</code> 圆角，支持浅/深双主题自适应。
      </p>
      <template #footer>
        <QnButton variant="ghost" @click="modalOpen = false">取消</QnButton>
        <QnButton variant="primary" @click="modalOpen = false">确认</QnButton>
      </template>
    </QnModalShell>
  </div>
</template>

<style scoped>
.qn-baseline {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-baseline__topbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--qn-space-4);
  flex-wrap: wrap;
}

.qn-baseline__heading {
  margin: 0;
  font-size: var(--qn-text-lg);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-baseline__subheading {
  margin: var(--qn-space-1) 0 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-baseline__theme-switch {
  display: flex;
  align-items: center;
  gap: var(--qn-space-2);
}

.qn-baseline__swatches {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: var(--qn-space-3);
}

.qn-baseline__swatch {
  display: flex;
  align-items: center;
  gap: var(--qn-space-3);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
}

.qn-baseline__swatch-dot {
  width: 28px;
  height: 28px;
  border-radius: var(--qn-radius-sm);
  border: 1px solid var(--qn-border-strong);
  flex-shrink: 0;
}

.qn-baseline__swatch-meta {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.qn-baseline__swatch-label {
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-primary);
}

.qn-baseline__swatch-token {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-baseline__type-scale {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-baseline__type-lg {
  font-size: var(--qn-text-lg);
  font-weight: 600;
}

.qn-baseline__type-md {
  font-size: var(--qn-text-md);
  font-weight: 600;
}

.qn-baseline__type-base {
  font-size: var(--qn-text-base);
}

.qn-baseline__type-sm {
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}

.qn-baseline__type-xs {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-baseline__mono {
  font-size: var(--qn-text-xs);
  padding: var(--qn-space-2) var(--qn-space-3);
  background-color: var(--qn-bg-sidebar);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
}

.qn-baseline__components {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-4);
}

.qn-baseline__row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--qn-space-2);
}

.qn-baseline__inputs {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: var(--qn-space-4);
}

.qn-baseline__banners {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-2);
}

.qn-baseline__modal-text {
  margin: 0;
  font-size: var(--qn-text-sm);
  color: var(--qn-text-secondary);
}
</style>
