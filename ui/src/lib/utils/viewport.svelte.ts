import { MediaQuery } from 'svelte/reactivity';

export const MOBILE_BREAKPOINT_PX = 768;

// Both fall back to `false` on the server so prerendered HTML is the desktop
// layout; a phone flips them on the first hydration tick.
export const isMobileViewport = new MediaQuery(`max-width: ${MOBILE_BREAKPOINT_PX - 1}px`, false);

export const isCoarsePointer = new MediaQuery('pointer: coarse', false);
