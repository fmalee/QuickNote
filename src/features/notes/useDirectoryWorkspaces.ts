import { ref } from 'vue';
import type { DirectoryWorkspaceConfig, DirectoryWorkspaceConfigEntry } from '@/bindings/types';
import {
  extractNotesErrorMessage,
  fetchDirectoryWorkspaceConfigMap,
  openDirectoryWorkspaceInVscode,
} from './api';

export interface NormalizedDirWorkspaceFields {
  vscodeWorkspacePath: string;
  projectRoot: string;
  defaultTerminalCwd: string;
}

export function extractDirWorkspaceFields(
  config: DirectoryWorkspaceConfig | null | undefined,
): NormalizedDirWorkspaceFields {
  return {
    vscodeWorkspacePath: (
      config?.vscodeWorkspacePath ??
      config?.vscode_workspace_path ??
      ''
    ).trim(),
    projectRoot: (config?.projectRoot ?? config?.project_root ?? '').trim(),
    defaultTerminalCwd: (config?.defaultTerminalCwd ?? config?.default_terminal_cwd ?? '').trim(),
  };
}

export function hasBoundDirWorkspaceConfig(
  entry: DirectoryWorkspaceConfigEntry | undefined,
): entry is DirectoryWorkspaceConfigEntry {
  if (!entry) {
    return false;
  }
  if (entry.parseError) {
    return true;
  }
  const fields = extractDirWorkspaceFields(entry.config);
  return Boolean(fields.vscodeWorkspacePath || fields.projectRoot || fields.defaultTerminalCwd);
}

export function formatDirWorkspaceTooltip(entry: DirectoryWorkspaceConfigEntry): string {
  if (entry.parseError) {
    return `工作区配置解析异常: ${entry.parseError}`;
  }
  const fields = extractDirWorkspaceFields(entry.config);
  const lines: string[] = [];
  if (fields.vscodeWorkspacePath) {
    lines.push(`VS Code 工作区: ${fields.vscodeWorkspacePath}`);
  }
  if (fields.projectRoot) {
    lines.push(`项目根目录: ${fields.projectRoot}`);
  }
  if (fields.defaultTerminalCwd) {
    lines.push(`默认终端目录: ${fields.defaultTerminalCwd}`);
  }
  return lines.join('\n') || '已绑定目录工作区配置';
}

export function useDirectoryWorkspaces(onError?: (message: string) => void) {
  const dirWorkspaceMap = ref<Record<string, DirectoryWorkspaceConfigEntry>>({});

  function getBoundDirWorkspaceEntry(relPath: string): DirectoryWorkspaceConfigEntry | null {
    const entry = dirWorkspaceMap.value[relPath];
    return hasBoundDirWorkspaceConfig(entry) ? entry : null;
  }

  function hasDirWorkspaceTarget(relPath: string): boolean {
    const entry = dirWorkspaceMap.value[relPath];
    if (!entry || entry.parseError || !entry.config) {
      return false;
    }
    const fields = extractDirWorkspaceFields(entry.config);
    return Boolean(fields.vscodeWorkspacePath || fields.projectRoot || fields.defaultTerminalCwd);
  }

  async function loadDirectoryWorkspaceConfigs(): Promise<void> {
    try {
      dirWorkspaceMap.value = await fetchDirectoryWorkspaceConfigMap();
    } catch {
      // 忽略目录工作区映射加载失败，避免阻断侧栏主渲染
    }
  }

  async function handleOpenDirWorkspaceInVscode(vaultRelDir: string): Promise<void> {
    const entry = dirWorkspaceMap.value[vaultRelDir];
    if (!entry?.config) {
      return;
    }
    try {
      await openDirectoryWorkspaceInVscode(entry.config);
    } catch (error) {
      onError?.(extractNotesErrorMessage(error));
    }
  }

  return {
    dirWorkspaceMap,
    getBoundDirWorkspaceEntry,
    hasDirWorkspaceTarget,
    formatDirWorkspaceTooltip,
    loadDirectoryWorkspaceConfigs,
    handleOpenDirWorkspaceInVscode,
  };
}
