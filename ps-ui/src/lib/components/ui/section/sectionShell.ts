import type { Snippet } from 'svelte';

export type SectionGroup = 'primary' | 'aside';

export type SectionDef = {
	id: string;
	title: string;
	header?: Snippet;
	body: Snippet;
	/** Only used by the `columns` presentation. */
	group: SectionGroup;
};

export type SectionPresentation = 'columns' | 'accordion' | 'tabs';

export function presentationFor(deviceClass: 'phone' | 'tablet' | 'desktop'): SectionPresentation {
	if (deviceClass === 'phone') return 'tabs';
	if (deviceClass === 'tablet') return 'accordion';
	return 'columns';
}
