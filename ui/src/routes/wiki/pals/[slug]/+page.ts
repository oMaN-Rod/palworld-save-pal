import { isDisabledRecord, toSlug } from '$lib/utils/wikiSlug';

export async function entries() {
	const palsJson = (await import('../../../../../../data/json/pals.json')).default;
	return Object.entries(palsJson as Record<string, unknown>)
		.filter(([, record]) => !isDisabledRecord(record))
		.map(([key]) => ({ slug: toSlug(key) }));
}

export function load({ params }: { params: { slug: string } }) {
	return { slug: params.slug };
}
