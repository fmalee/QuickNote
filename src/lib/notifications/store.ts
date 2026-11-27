import { ref } from 'vue';
import { defineStore } from 'pinia';

export type NotificationTone = 'info' | 'success' | 'warning' | 'danger';

export interface StatusNotification {
  id: number;
  tone: NotificationTone;
  message: string;
  durationMs: number;
}

export interface ToastNotification {
  id: number;
  tone: NotificationTone;
  title?: string;
  message: string;
  durationMs: number;
}

export interface NotifyStatusInput {
  tone?: NotificationTone;
  message: string;
  durationMs?: number;
}

export interface NotifyToastInput {
  tone?: NotificationTone;
  title?: string;
  message: string;
  durationMs?: number;
}

const DEFAULT_STATUS_DURATION_MS: Record<NotificationTone, number> = {
  info: 3500,
  success: 3500,
  warning: 5500,
  danger: 5500,
};

const DEFAULT_TOAST_DURATION_MS: Record<NotificationTone, number> = {
  info: 4500,
  success: 4500,
  warning: 6000,
  danger: 7000,
};

const MAX_VISIBLE_TOASTS = 4;

export const useNotificationStore = defineStore('notifications', () => {
  const statusNotice = ref<StatusNotification | null>(null);
  const toasts = ref<ToastNotification[]>([]);

  let seq = 0;
  let statusTimer: ReturnType<typeof setTimeout> | null = null;
  const toastTimers = new Map<number, ReturnType<typeof setTimeout>>();

  function clearStatusTimer(): void {
    if (statusTimer !== null) {
      clearTimeout(statusTimer);
      statusTimer = null;
    }
  }

  function scheduleStatusTimer(id: number, durationMs: number): void {
    clearStatusTimer();
    if (durationMs <= 0) {
      return;
    }
    statusTimer = setTimeout(() => {
      statusTimer = null;
      if (statusNotice.value?.id === id) {
        statusNotice.value = null;
      }
    }, durationMs);
  }

  function notifyStatus(input: NotifyStatusInput | string): number {
    const normalized: NotifyStatusInput = typeof input === 'string' ? { message: input } : input;
    const message = normalized.message.trim();
    if (!message) {
      return 0;
    }
    const tone: NotificationTone = normalized.tone ?? 'info';
    const durationMs = normalized.durationMs ?? DEFAULT_STATUS_DURATION_MS[tone];
    const id = ++seq;
    statusNotice.value = { id, tone, message, durationMs };
    scheduleStatusTimer(id, durationMs);
    return id;
  }

  function pauseStatusTimer(): void {
    clearStatusTimer();
  }

  function resumeStatusTimer(): void {
    const current = statusNotice.value;
    if (!current || current.durationMs <= 0) {
      return;
    }
    scheduleStatusTimer(current.id, Math.min(current.durationMs, 2000));
  }

  function clearStatus(): void {
    clearStatusTimer();
    statusNotice.value = null;
  }

  function clearToastTimer(id: number): void {
    const timer = toastTimers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      toastTimers.delete(id);
    }
  }

  function scheduleToastTimer(id: number, durationMs: number): void {
    clearToastTimer(id);
    if (durationMs <= 0) {
      return;
    }
    const timer = setTimeout(() => {
      toastTimers.delete(id);
      toasts.value = toasts.value.filter((item) => item.id !== id);
    }, durationMs);
    toastTimers.set(id, timer);
  }

  function dismissToast(id: number): void {
    clearToastTimer(id);
    toasts.value = toasts.value.filter((item) => item.id !== id);
  }

  function notifyToast(input: NotifyToastInput): number {
    const message = input.message.trim();
    if (!message) {
      return 0;
    }
    const tone: NotificationTone = input.tone ?? 'info';
    const title = input.title?.trim() || undefined;
    const durationMs = input.durationMs ?? DEFAULT_TOAST_DURATION_MS[tone];

    const duplicate = toasts.value.find(
      (item) => item.tone === tone && item.title === title && item.message === message,
    );
    if (duplicate) {
      duplicate.durationMs = durationMs;
      scheduleToastTimer(duplicate.id, durationMs);
      return duplicate.id;
    }

    const id = ++seq;
    const nextItem: ToastNotification = { id, tone, title, message, durationMs };
    const nextList = [...toasts.value, nextItem];
    while (nextList.length > MAX_VISIBLE_TOASTS) {
      const removed = nextList.shift();
      if (removed) {
        clearToastTimer(removed.id);
      }
    }
    toasts.value = nextList;
    scheduleToastTimer(id, durationMs);
    return id;
  }

  function pauseToastTimer(id: number): void {
    clearToastTimer(id);
  }

  function resumeToastTimer(id: number): void {
    const target = toasts.value.find((item) => item.id === id);
    if (!target || target.durationMs <= 0) {
      return;
    }
    scheduleToastTimer(target.id, Math.min(target.durationMs, 2500));
  }

  function clearToasts(): void {
    for (const id of toastTimers.keys()) {
      clearToastTimer(id);
    }
    toasts.value = [];
  }

  function dispose(): void {
    clearStatus();
    clearToasts();
  }

  return {
    statusNotice,
    toasts,
    notifyStatus,
    pauseStatusTimer,
    resumeStatusTimer,
    clearStatus,
    notifyToast,
    dismissToast,
    pauseToastTimer,
    resumeToastTimer,
    clearToasts,
    dispose,
  };
});

export function pushStatusNotice(input: NotifyStatusInput | string): number {
  try {
    return useNotificationStore().notifyStatus(input);
  } catch {
    return 0;
  }
}

export function pushToastNotice(input: NotifyToastInput): number {
  try {
    return useNotificationStore().notifyToast(input);
  } catch {
    return 0;
  }
}
