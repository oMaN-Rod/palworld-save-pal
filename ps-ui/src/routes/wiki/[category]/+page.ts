import { error } from '@sveltejs/kit';
import { WIKI_CATEGORIES, type WikiCategory } from '$lib/utils/wikiCategories';
import { loadCategorySeo } from '$lib/utils/wikiL10n';
import { stripKeyPrefix, toSlug } from '$lib/utils/wikiSlug';

export const ssr = true;
export const prerender = true;

const CATEGORIES = WIKI_CATEGORIES.map((category) => category.id).filter(
	(id) => id !== 'pals'
) as WikiCategory[];

export function entries() {
	return CATEGORIES.map((category) => ({ category }));
}

export async function load({ params }: { params: { category: string } }) {
	const category = params.category as WikiCategory;
	if (!CATEGORIES.includes(category)) {
		error(404, 'Not found');
	}
	const entities = await loadCategorySeo(category);
	return {
		category,
		names: entities.map((entity) => entity.name),
		slugs: entities.map((entity) => toSlug(stripKeyPrefix(entity.key)))
	};
}
