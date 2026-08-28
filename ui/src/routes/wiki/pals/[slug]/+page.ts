import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
import { loadEntitySeo } from '$lib/utils/wikiL10n';
import { isHiddenRecord, toSlug } from '$lib/utils/wikiSlug';

export const ssr = true;
export const prerender = PUBLIC_DESKTOP_MODE !== 'true';

export async function entries() {
	const palsJson = (await import('../../../../../../data/json/pals.json')).default;
	return Object.entries(palsJson as Record<string, unknown>)
		.filter(([, record]) => !isHiddenRecord(record))
		.map(([key]) => ({ slug: toSlug(key) }));
}

export async function load({ params }: { params: { slug: string } }) {
	const entity = await loadEntitySeo('pals', params.slug);
	return {
		slug: params.slug,
		name: entity?.name ?? params.slug,
		description: entity?.description ?? null
	};
}
