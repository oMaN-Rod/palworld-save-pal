import { error } from '@sveltejs/kit';
import { descriptorFor } from '$lib/utils/wikiDescriptors';
import { WIKI_CATEGORIES, type WikiCategory } from '$lib/utils/wikiCategories';
import { stripKeyPrefix, toSlug } from '$lib/utils/wikiSlug';

const CATEGORIES = WIKI_CATEGORIES.map((category) => category.id).filter(
	(id) => id !== 'pals'
) as WikiCategory[];

export async function entries() {
	const results: { category: WikiCategory; slug: string }[] = [];
	for (const category of CATEGORIES) {
		const json = await descriptorFor(category).loadJson();
		for (const key of Object.keys(json)) {
			results.push({ category, slug: toSlug(stripKeyPrefix(key)) });
		}
	}
	return results;
}

export function load({ params }: { params: { category: string; slug: string } }) {
	const category = params.category as WikiCategory;
	if (!CATEGORIES.includes(category)) {
		error(404, 'Not found');
	}
	return { category, slug: params.slug };
}
