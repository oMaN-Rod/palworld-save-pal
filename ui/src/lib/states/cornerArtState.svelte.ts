import { persistedState } from 'svelte-persisted-state';

/**
 * Whether the ambient Palworld corner art (bg-corner.webp, bottom right of the
 * app background) is shown. Persisted to localStorage; toggled from the
 * settings modal and read by the root layout.
 */
export const cornerArt = persistedState<boolean>('psp-corner-art', true);
