import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { mockHideQuickBarWindow } = vi.hoisted(() => ({
  mockHideQuickBarWindow: vi.fn(),
}));

vi.mock('../api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api')>();
  return {
    ...actual,
    hideQuickBarWindow: mockHideQuickBarWindow,
  };
});

vi.mock('@/lib/ipc/events', () => ({
  onWindowState: vi.fn().mockResolvedValue(() => {}),
  onSettingsStatus: vi.fn().mockResolvedValue(() => {}),
  onToastAppendUndo: vi.fn().mockResolvedValue(() => {}),
  onFsExternalChange: vi.fn().mockResolvedValue(() => {}),
  listenEvent: vi.fn().mockResolvedValue(() => {}),
}));

import { useQuickBarStore } from '../store';

describe('quickbar handleEscape (R33)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockHideQuickBarWindow.mockReset();
    mockHideQuickBarWindow.mockResolvedValue(undefined);
  });

  it('hides Quick Bar directly with reason esc and resets mode to capture when in open mode', async () => {
    const store = useQuickBarStore();
    store.mode = 'open';

    await store.handleEscape();

    expect(mockHideQuickBarWindow).toHaveBeenCalledTimes(1);
    expect(mockHideQuickBarWindow).toHaveBeenCalledWith('esc');
    expect(store.mode).toBe('capture');
  });

  it('hides Quick Bar directly with reason esc and resets mode to capture when in command mode', async () => {
    const store = useQuickBarStore();
    store.mode = 'command';

    await store.handleEscape();

    expect(mockHideQuickBarWindow).toHaveBeenCalledTimes(1);
    expect(mockHideQuickBarWindow).toHaveBeenCalledWith('esc');
    expect(store.mode).toBe('capture');
  });

  it('only closes target picker without hiding when target picker is open', async () => {
    const store = useQuickBarStore();
    store.targetPickerOpen = true;

    await store.handleEscape();

    expect(store.targetPickerOpen).toBe(false);
    expect(mockHideQuickBarWindow).not.toHaveBeenCalled();
  });

  it('only closes save-as modal without hiding when save-as modal is open', async () => {
    const store = useQuickBarStore();
    store.saveAsModalOpen = true;

    await store.handleEscape();

    expect(store.saveAsModalOpen).toBe(false);
    expect(mockHideQuickBarWindow).not.toHaveBeenCalled();
  });
});
