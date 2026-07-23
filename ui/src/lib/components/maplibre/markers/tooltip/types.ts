import type { Snippet } from 'svelte';
import type { TooltipFeature } from '../../types.js';

export interface TooltipProps {
	layers?: string[];
	offset?: { x: number; y: number };
	content?: Snippet<[TooltipFeature]>;
	class?: string;
}
