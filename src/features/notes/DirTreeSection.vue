<script setup lang="ts">
import { computed, ref } from 'vue';
import type { DirNode, DirectoryWorkspaceConfigEntry } from '@/bindings/types';
import { type ContextMenuEntry, QnBadge, QnContextMenu } from '@/components';
import { type ActionContext, actionRegistry } from '@/lib/actions';
import {
  extractDirWorkspaceFields,
  formatDirWorkspaceTooltip,
  hasBoundDirWorkspaceConfig,
} from './useDirectoryWorkspaces';

interface FlatDirItem {
  node: DirNode;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
}

const FOLDER_CONTEXT_ACTION_IDS = [
  'folder.bindWorkspaceConfig',
  'folder.openWorkspaceInVscode',
] as const;

const props = defineProps<{
  dirs: DirNode[];
  selectedDir: string | null;
  dirWorkspaceMap: Record<string, DirectoryWorkspaceConfigEntry>;
}>();

const emit = defineEmits<{
  (e: 'select-dir', relPath: string | null): void;
}>();

const expandedDirs = ref<Record<string, boolean>>({});
const dirCtxOpen = ref<boolean>(false);
const dirCtxPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const dirCtxTarget = ref<DirNode | null>(null);

function getBoundDirWorkspaceEntry(relPath: string): DirectoryWorkspaceConfigEntry | null {
  const entry = props.dirWorkspaceMap[relPath];
  return hasBoundDirWorkspaceConfig(entry) ? entry : null;
}

function hasDirWorkspaceTarget(relPath: string): boolean {
  const entry = props.dirWorkspaceMap[relPath];
  if (!entry || entry.parseError || !entry.config) {
    return false;
  }
  const fields = extractDirWorkspaceFields(entry.config);
  return Boolean(fields.vscodeWorkspacePath || fields.projectRoot || fields.defaultTerminalCwd);
}

function buildDirActionContext(node: DirNode | null): ActionContext {
  return {
    activeScope: 'list',
    payload: node
      ? {
          vaultRelDir: node.relPath,
          dirName: node.name,
          hasWorkspaceTarget: hasDirWorkspaceTarget(node.relPath),
        }
      : {},
  };
}

const dirCtxMenuItems = computed<ContextMenuEntry[]>(() => {
  const ctx = buildDirActionContext(dirCtxTarget.value);
  const entries: ContextMenuEntry[] = [];
  for (const id of FOLDER_CONTEXT_ACTION_IDS) {
    const def = actionRegistry.getById(id);
    if (!def) {
      continue;
    }
    const disabled = def.when ? !def.when(ctx) : false;
    entries.push({
      type: 'item',
      id: def.id,
      title: def.title,
      disabled,
      disabledReason:
        disabled && def.id === 'folder.openWorkspaceInVscode'
          ? '请先为该目录绑定工作区路径或项目根目录'
          : null,
    });
  }
  return entries;
});

const flatDirs = computed<FlatDirItem[]>(() => {
  const result: FlatDirItem[] = [];
  function walk(nodes: DirNode[], depth: number): void {
    for (const node of nodes) {
      const hasChildren = node.children.length > 0;
      const expanded = expandedDirs.value[node.relPath] ?? depth === 0;
      result.push({ node, depth, hasChildren, expanded });
      if (hasChildren && expanded) {
        walk(node.children, depth + 1);
      }
    }
  }
  walk(props.dirs, 0);
  return result;
});

function toggleDirExpand(relPath: string): void {
  expandedDirs.value = {
    ...expandedDirs.value,
    [relPath]: !(expandedDirs.value[relPath] ?? true),
  };
}

function openDirContextMenu(event: MouseEvent, node: DirNode): void {
  dirCtxTarget.value = node;
  dirCtxPos.value = { x: event.clientX, y: event.clientY };
  dirCtxOpen.value = true;
}

async function handleDirContextSelect(id: string): Promise<void> {
  const target = dirCtxTarget.value;
  dirCtxOpen.value = false;
  if (!target) {
    return;
  }
  await actionRegistry.executeById(id, buildDirActionContext(target));
}
</script>

<template>
  <div class="qn-notes-sidebar__subpanel" data-testid="nav-projects-tree">
    <div class="qn-notes-sidebar__subpanel-header">
      <span>目录</span>
      <button
        v-if="props.selectedDir"
        type="button"
        class="qn-notes-sidebar__link-btn"
        @click="emit('select-dir', null)"
      >
        ← 根目录
      </button>
    </div>
    <div v-if="flatDirs.length === 0" class="qn-notes-sidebar__empty">暂无子目录</div>
    <div v-else class="qn-notes-sidebar__tree">
      <div
        v-for="entry in flatDirs"
        :key="entry.node.relPath"
        :style="{ paddingLeft: `calc(var(--qn-space-2) + ${entry.depth} * var(--qn-space-3))` }"
        :class="[
          'qn-notes-sidebar__tree-item',
          {
            'qn-notes-sidebar__tree-item--active': props.selectedDir === entry.node.relPath,
          },
        ]"
        :data-testid="`dir-node-${entry.node.relPath}`"
        @click="emit('select-dir', entry.node.relPath)"
        @contextmenu.prevent="openDirContextMenu($event, entry.node)"
      >
        <button
          v-if="entry.hasChildren"
          type="button"
          class="qn-notes-sidebar__expand-btn"
          :aria-label="entry.expanded ? '折叠目录' : '展开目录'"
          @click.stop="toggleDirExpand(entry.node.relPath)"
        >
          {{ entry.expanded ? '▾' : '▸' }}
        </button>
        <span v-else class="qn-notes-sidebar__expand-spacer" />
        <span class="qn-notes-sidebar__tree-label">{{ entry.node.name }}</span>
        <QnBadge
          v-if="getBoundDirWorkspaceEntry(entry.node.relPath)"
          :tone="getBoundDirWorkspaceEntry(entry.node.relPath)?.parseError ? 'warning' : 'accent'"
          size="sm"
          class="qn-notes-sidebar__ws-badge"
          :title="formatDirWorkspaceTooltip(getBoundDirWorkspaceEntry(entry.node.relPath)!)"
          :data-testid="`dir-ws-badge-${entry.node.relPath}`"
        >
          {{ getBoundDirWorkspaceEntry(entry.node.relPath)?.parseError ? '异常' : 'WS' }}
        </QnBadge>
        <span class="qn-notes-sidebar__tree-count">{{ entry.node.count }}</span>
      </div>
    </div>
    <QnContextMenu
      :open="dirCtxOpen"
      :x="dirCtxPos.x"
      :y="dirCtxPos.y"
      :items="dirCtxMenuItems"
      @select="handleDirContextSelect"
      @close="dirCtxOpen = false"
    />
  </div>
</template>

<style scoped>
.qn-notes-sidebar__subpanel {
  display: flex;
  flex-direction: column;
  gap: var(--qn-space-1);
  padding: var(--qn-space-2);
  border: 1px solid var(--qn-border-subtle);
  border-radius: var(--qn-radius-sm);
  background-color: var(--qn-bg-base);
  max-height: 180px;
  overflow-y: auto;
}

.qn-notes-sidebar__subpanel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--qn-text-xs);
  font-weight: 600;
  color: var(--qn-text-secondary);
}

.qn-notes-sidebar__link-btn {
  border: none;
  background: transparent;
  padding: 0;
  font-size: var(--qn-text-xs);
  color: var(--qn-accent);
  cursor: pointer;
}

.qn-notes-sidebar__empty {
  padding: var(--qn-space-4) var(--qn-space-2);
  text-align: center;
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__tree {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.qn-notes-sidebar__tree-item {
  display: flex;
  align-items: center;
  gap: var(--qn-space-1);
  padding: var(--qn-space-1) var(--qn-space-2);
  border-radius: var(--qn-radius-sm);
  font-size: var(--qn-text-xs);
  color: var(--qn-text-secondary);
  cursor: pointer;
}

.qn-notes-sidebar__tree-item:hover {
  background-color: var(--qn-bg-hover);
  color: var(--qn-text-primary);
}

.qn-notes-sidebar__tree-item--active {
  background-color: var(--qn-accent-soft);
  color: var(--qn-accent);
}

.qn-notes-sidebar__expand-btn {
  width: 14px;
  height: 14px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--qn-text-muted);
  font-size: var(--qn-text-xs);
  cursor: pointer;
  flex-shrink: 0;
}

.qn-notes-sidebar__expand-spacer {
  width: 14px;
  flex-shrink: 0;
}

.qn-notes-sidebar__tree-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.qn-notes-sidebar__tree-count {
  font-size: var(--qn-text-xs);
  color: var(--qn-text-muted);
}

.qn-notes-sidebar__ws-badge {
  flex-shrink: 0;
}
</style>
