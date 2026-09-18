import { persistedState } from 'svelte-persisted-state';

export type ModsView = 'grid' | 'list';

export const modsViewMode = persistedState<ModsView>('ps-mods-view', 'grid');
