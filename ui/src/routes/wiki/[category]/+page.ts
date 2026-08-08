import { error } from '@sveltejs/kit';
import { WIKI_CATEGORIES, type WikiCategory } from '$lib/utils/wikiCategories';

const CATEGORIES = WIKI_CATEGORIES.map((category) => category.id).filter(
	(id) => id !== 'pals'
) as WikiCategory[];

export function entries() {
	return CATEGORIES.map((category) => ({ category }));
}

export function load({ params }: { params: { category: string } }) {
	const category = params.category as WikiCategory;
	if (!CATEGORIES.includes(category)) {
		error(404, 'Not found');
	}
	return { category };
}
