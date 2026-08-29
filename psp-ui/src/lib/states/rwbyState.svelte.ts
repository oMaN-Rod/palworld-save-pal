import { persistedState } from 'svelte-persisted-state';

/**
 * The Signal tab's RWBY easter egg, app-wide. `rwbyUnlocked` flips once the
 * lore overlay hears the team's name typed letter by letter (and flips back
 * via "Hide the secret again"); `rwbySkin` toggles the RWBY palette over the
 * whole WebUI. Persisted to localStorage; the `.rwby-skin` class on <body>
 * that actually swaps the palette is kept in sync from the root layout.
 */
export const rwbyUnlocked = persistedState<boolean>('psp-rwby-unlocked', false);
export const rwbySkin = persistedState<boolean>('psp-rwby-skin', false);
