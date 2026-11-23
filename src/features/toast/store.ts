import { ref } from 'vue';
import { defineStore } from 'pinia';
import type { UnlistenFn } from '@/lib/ipc/events';
import { onToastAppendUndo, type ToastAppendUndoPayload } from './api';

export const TOAST_DURATION_SECONDS = 5;

export const useToastStore = defineStore('toast', () => {
  const visible = ref<boolean>(false);
  const payload = ref<ToastAppendUndoPayload | null>(null);
  const remainingSeconds = ref<number>(TOAST_DURATION_SECONDS);
  const undoTriggered = ref<boolean>(false);

  let unlistenFn: UnlistenFn | null = null;
  let countdownTimer: ReturnType<typeof setInterval> | null = null;

  function clearCountdown(): void {
    if (countdownTimer !== null) {
      clearInterval(countdownTimer);
      countdownTimer = null;
    }
  }

  function dismiss(): void {
    clearCountdown();
    visible.value = false;
  }

  function showPayload(nextPayload: ToastAppendUndoPayload): void {
    clearCountdown();
    payload.value = nextPayload;
    undoTriggered.value = false;
    remainingSeconds.value = TOAST_DURATION_SECONDS;
    visible.value = true;

    countdownTimer = setInterval(() => {
      if (remainingSeconds.value <= 1) {
        dismiss();
      } else {
        remainingSeconds.value -= 1;
      }
    }, 1000);
  }

  function triggerUndo(): void {
    undoTriggered.value = true;
    dismiss();
  }

  async function initialize(): Promise<void> {
    if (unlistenFn) {
      return;
    }
    unlistenFn = await onToastAppendUndo((eventPayload) => {
      showPayload(eventPayload);
    });
  }

  function dispose(): void {
    clearCountdown();
    if (unlistenFn) {
      unlistenFn();
      unlistenFn = null;
    }
  }

  return {
    visible,
    payload,
    remainingSeconds,
    undoTriggered,
    showPayload,
    triggerUndo,
    dismiss,
    initialize,
    dispose,
  };
});
