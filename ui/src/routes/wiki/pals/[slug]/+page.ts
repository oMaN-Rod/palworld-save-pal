import { toSlug } from '$lib/utils/wikiSlug';

export async function entries() {
	const palsJson = (await import('../../../../../../data/json/pals.json')).default;
	const keys = Object.keys(palsJson as Record<string, unknown>);
	return keys.map((key) => ({ slug: toSlug(key) }));
}

export function load({ params }: { params: { slug: string } }) {
	return { slug: params.slug };
}
