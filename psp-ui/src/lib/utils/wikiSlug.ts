export function toSlug(key: string): string {
	return key
		.replace(/([a-z0-9])([A-Z])/g, '$1-$2')
		.replace(/[_\s]+/g, '-')
		.toLowerCase()
		.replace(/[^a-z0-9-]+/g, '-')
		.replace(/-+/g, '-')
		.replace(/^-|-$/g, '');
}

export function buildSlugIndex(keys: string[]): Map<string, string> {
	const index = new Map<string, string>();
	for (const key of keys) {
		const slug = toSlug(key);
		const existing = index.get(slug);
		if (existing !== undefined) {
			throw new Error(`Slug collision: "${key}" and "${existing}" both produce "${slug}"`);
		}
		index.set(slug, key);
	}
	return index;
}

export function keyFromSlug(slug: string, index: Map<string, string>): string | undefined {
	return index.get(slug.toLowerCase());
}

export function stripKeyPrefix(key: string): string {
	const index = key.lastIndexOf('::');
	return index === -1 ? key : key.slice(index + 2);
}

// Lives here rather than alongside the descriptors so build-time route modules
// can filter without importing the runtime data stores.
//
// Two distinct reasons a record has no wiki entry of its own:
//   `disabled`    - the parser could not resolve the record (a missing icon row, a placeholder
//                   name), so there is nothing to show.
//   `redirect_to` - the game retires this id onto a surviving one when it loads a save. The
//                   two share a name and a stat block, so listing both is a duplicate whose
//                   page describes the survivor anyway.
// Only retired ids carry `redirect_to`; the rarity variants of an item (ClothArmor_2..5 and
// the rest) are live rows with pages of their own.
export function isHiddenRecord(record: unknown): boolean {
	if (!record || typeof record !== 'object') return false;
	const value = record as Record<string, unknown>;
	if (value.disabled === true || typeof value.redirect_to === 'string') return true;
	const details = value.details;
	if (!details || typeof details !== 'object') return false;
	const detail = details as Record<string, unknown>;
	return detail.disabled === true || typeof detail.redirect_to === 'string';
}

