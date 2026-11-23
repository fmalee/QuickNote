import { registerDefaultActionCatalog } from './catalog';
import { createActionRegistry } from './registry';

export * from './types';
export * from './keymap';
export * from './registry';
export * from './catalog';

export const actionRegistry = createActionRegistry();
registerDefaultActionCatalog(actionRegistry);
