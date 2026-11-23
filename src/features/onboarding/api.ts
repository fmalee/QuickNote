import { isAppError, libraryGetInfo, librarySetRoot, windowShowMain } from '@/lib/ipc/commands';
import type { LibraryInfo, SetRootArgs, WindowState } from '@/bindings/types';

export function fetchLibraryInfo(): Promise<LibraryInfo> {
  return libraryGetInfo();
}

export function configureLibraryRoot(args: SetRootArgs): Promise<LibraryInfo> {
  return librarySetRoot(args);
}

export function openMainWindow(): Promise<WindowState> {
  return windowShowMain({ mode: null });
}

export function extractOnboardingErrorMessage(error: unknown): string {
  if (isAppError(error)) {
    return error.detail ? `${error.message}（${error.detail}）` : error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return '初始化笔记库失败，请检查路径权限后重试。';
}
