import { MediaQuery } from 'svelte/reactivity';

export type DeviceClass = 'phone' | 'tablet' | 'desktop';

/** Tailwind's `md` and `xl` boundaries, which the existing classes depend on. */
export const BREAKPOINTS = { md: 768, xl: 1280 } as const;

// Desktop on the server; a phone flips on hydration, so nothing behind these may mount WebGL or fetch on first render.
const belowMd = new MediaQuery(`max-width: ${BREAKPOINTS.md - 1}px`, false);
const belowXl = new MediaQuery(`max-width: ${BREAKPOINTS.xl - 1}px`, false);
const coarsePointer = new MediaQuery('pointer: coarse', false);
const portraitOrientation = new MediaQuery('orientation: portrait', false);

export const layout = {
	get phone(): boolean {
		return belowMd.current;
	},
	get tablet(): boolean {
		return belowXl.current && !belowMd.current;
	},
	get desktop(): boolean {
		return !belowXl.current;
	},
	get coarse(): boolean {
		return coarsePointer.current;
	},
	get portrait(): boolean {
		return portraitOrientation.current;
	},
	get deviceClass(): DeviceClass {
		if (belowMd.current) return 'phone';
		if (belowXl.current) return 'tablet';
		return 'desktop';
	}
};
