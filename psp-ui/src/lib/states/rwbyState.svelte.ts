import { persistedState } from 'svelte-persisted-state';

export const rwbyUnlocked = persistedState<boolean>('psp-rwby-unlocked', false);
export const rwbySkin = persistedState<boolean>('psp-rwby-skin', false);

export const ROSE_RED = '#a42227';
