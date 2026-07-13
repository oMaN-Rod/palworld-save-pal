import { persistedState } from 'svelte-persisted-state';

/**
 * Latest GitHub release version the update-available modal has already been
 * shown for, persisted so it doesn't re-nag on every reconnect/reload.
 */
export const dismissedUpdateVersion = persistedState<string>('psp-dismissed-update-version', '');
