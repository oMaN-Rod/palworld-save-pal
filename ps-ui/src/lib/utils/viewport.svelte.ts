// Legacy import path; new code should import `layout` from './layout.svelte'.
import { MediaQuery } from 'svelte/reactivity';

import { BREAKPOINTS } from './layout.svelte';

export const isMobileViewport = new MediaQuery(`max-width: ${BREAKPOINTS.md - 1}px`, false);

export const isCoarsePointer = new MediaQuery('pointer: coarse', false);
