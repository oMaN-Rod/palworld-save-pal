import * as m from '$i18n/messages';
import { c } from './commonTranslations';
import { stripKeyPrefix, toSlug } from './wikiSlug';

export type WikiCategory =
	| 'pals'
	| 'items'
	| 'buildings'
	| 'active-skills'
	| 'passive-skills'
	| 'technologies'
	| 'elements'
	| 'work-suitability';

export type WikiCategoryDef = {
	id: WikiCategory;
	/** Lazily called so a locale change is picked up on next render. */
	label: () => string;
};

export const WIKI_CATEGORIES: WikiCategoryDef[] = [
	{ id: 'pals', label: () => c.pal },
	{ id: 'items', label: () => c.item },
	{ id: 'buildings', label: () => 'Buildings' },
	{ id: 'active-skills', label: () => m.active_skill({ count: 2 }) },
	{ id: 'passive-skills', label: () => m.passive_skill({ count: 2 }) },
	{ id: 'technologies', label: () => m.technology({ count: 2 }) },
	{ id: 'elements', label: () => 'Elements' },
	{ id: 'work-suitability', label: () => 'Work Suitability' }
];

export function categoryLabel(category: WikiCategory): string {
	return WIKI_CATEGORIES.find((c) => c.id === category)?.label() ?? category;
}

/**
 * Plural label for headings and page titles. `categoryLabel` stays singular for
 * `pals`/`items` because breadcrumbs and chips read better that way.
 */
export function categoryLabelPlural(category: WikiCategory): string {
	if (category === 'pals') return m.pal({ count: 2 });
	if (category === 'items') return m.item({ count: 2 });
	return categoryLabel(category);
}

export function categoryHref(category: WikiCategory): string {
	return `/wiki/${category}`;
}

export function entityLink(category: WikiCategory, key: string): { href: string } {
	return { href: `/wiki/${category}/${toSlug(stripKeyPrefix(key))}` };
}
