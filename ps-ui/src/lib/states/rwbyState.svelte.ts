import { persistedState } from 'svelte-persisted-state';

export const rwbyUnlocked = persistedState<boolean>('ps-rwby-unlocked', false);
export const rwbySkin = persistedState<boolean>('ps-rwby-skin', false);

export const ROSE_RED = '#a42227';
