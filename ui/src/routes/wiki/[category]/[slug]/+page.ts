import { error } from '@sveltejs/kit';
import { descriptorFor } from '$lib/utils/wikiDescriptors';
import { WIKI_CATEGORIES, type WikiCategory } from '$lib/utils/wikiCategories';
import { loadEntitySeo } from '$lib/utils/wikiL10n';
import { isDisabledRecord, stripKeyPrefix, toSlug } from '$lib/utils/wikiSlug';

export const ssr = true;
export const prerender = true;

const CATEGORIES = WIKI_CATEGORIES.map((category) => category.id).filter(
	(id) => id !== 'pals'
) as WikiCategory[];

export async function entries() {
	const results: { category: WikiCategory; slug: string }[] = [];
	for (const category of CATEGORIES) {
		const json = await descriptorFor(category).loadJson();
		for (const [key, record] of Object.entries(json)) {
			if (isDisabledRecord(record)) continue;
			results.push({ category, slug: toSlug(stripKeyPrefix(key)) });
		}
	}
	return results;
}

export async function load({ params }: { params: { category: string; slug: string } }) {
	const category = params.category as WikiCategory;
	if (!CATEGORIES.includes(category)) {
		error(404, 'Not found');
	}
	const entity = await loadEntitySeo(category, params.slug, { stripPrefix: true });
	return {
		category,
		slug: params.slug,
		name: entity?.name ?? params.slug,
		description: entity?.description ?? null
	};
}
