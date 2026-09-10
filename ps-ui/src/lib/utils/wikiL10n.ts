import type { WikiCategory } from './wikiCategories';
import { isHiddenRecord, stripKeyPrefix, toSlug } from './wikiSlug';

export type EntitySeo = { key: string; name: string; description: string | null };

type L10nRecord = { localized_name?: string | null; description?: string | null };

// Import specifiers are static (not built from `category`) so Vite can analyse them.
const RAW_LOADERS: Record<WikiCategory, () => Promise<Record<string, unknown>>> = {
	pals: async () => (await import('../../../../data/json/pals.json')).default,
	items: async () => (await import('../../../../data/json/items.json')).default,
	buildings: async () => (await import('../../../../data/json/buildings.json')).default,
	'active-skills': async () => (await import('../../../../data/json/active_skills.json')).default,
	'passive-skills': async () => (await import('../../../../data/json/passive_skills.json')).default,
	technologies: async () => (await import('../../../../data/json/technologies.json')).default,
	elements: async () => (await import('../../../../data/json/elements.json')).default,
	'work-suitability': async () =>
		(await import('../../../../data/json/l10n/en/work_suitability.json')).default
};

const L10N_LOADERS: Record<WikiCategory, () => Promise<Record<string, L10nRecord>>> = {
	pals: async () => (await import('../../../../data/json/l10n/en/pals.json')).default,
	items: async () => (await import('../../../../data/json/l10n/en/items.json')).default,
	buildings: async () => (await import('../../../../data/json/l10n/en/buildings.json')).default,
	'active-skills': async () =>
		(await import('../../../../data/json/l10n/en/active_skills.json')).default,
	'passive-skills': async () =>
		(await import('../../../../data/json/l10n/en/passive_skills.json')).default,
	technologies: async () =>
		(await import('../../../../data/json/l10n/en/technologies.json')).default,
	elements: async () => (await import('../../../../data/json/l10n/en/elements.json')).default,
	'work-suitability': async () =>
		(await import('../../../../data/json/l10n/en/work_suitability.json')).default
};

const rawCache = new Map<WikiCategory, Promise<Record<string, unknown>>>();
const l10nCache = new Map<WikiCategory, Promise<Record<string, L10nRecord>>>();

function cached<T>(
	cache: Map<WikiCategory, Promise<T>>,
	loaders: Record<WikiCategory, () => Promise<T>>,
	category: WikiCategory
): Promise<T> {
	let pending = cache.get(category);
	if (!pending) {
		pending = loaders[category]();
		cache.set(category, pending);
	}
	return pending;
}

async function enabledKeys(category: WikiCategory): Promise<string[]> {
	const json = await cached(rawCache, RAW_LOADERS, category);
	return Object.entries(json)
		.filter(([, record]) => !isHiddenRecord(record))
		.map(([key]) => key);
}

function toEntity(key: string, l10n: Record<string, L10nRecord>): EntitySeo {
	const record = l10n[key];
	const name = record?.localized_name;
	const description = record?.description;
	return {
		key,
		name: typeof name === 'string' && name.length > 0 ? name : stripKeyPrefix(key),
		description: typeof description === 'string' && description.length > 0 ? description : null
	};
}

export async function loadEntitySeo(
	category: WikiCategory,
	slug: string,
	options: { stripPrefix?: boolean } = {}
): Promise<EntitySeo | null> {
	const stripPrefix = options.stripPrefix ?? false;
	const [keys, l10n] = await Promise.all([
		enabledKeys(category),
		cached(l10nCache, L10N_LOADERS, category)
	]);
	const target = slug.toLowerCase();
	const key = keys.find(
		(candidate) => toSlug(stripPrefix ? stripKeyPrefix(candidate) : candidate) === target
	);
	return key ? toEntity(key, l10n) : null;
}

export async function loadCategorySeo(category: WikiCategory): Promise<EntitySeo[]> {
	const [keys, l10n] = await Promise.all([
		enabledKeys(category),
		cached(l10nCache, L10N_LOADERS, category)
	]);
	return keys.map((key) => toEntity(key, l10n));
}
