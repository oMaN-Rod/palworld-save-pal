export function toSlug(key: string): string {
	return key
		.replace(/([a-z0-9])([A-Z])/g, '$1-$2')
		.replace(/[_\s]+/g, '-')
		.toLowerCase()
		.replace(/[^a-z0-9-]+/g, '-')
		.replace(/-+/g, '-')
		.replace(/^-|-$/g, '');
}

// `toSlug` discards case and separators, so the reverse direction is a lookup
// over the known key set rather than an inverse transform.
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

// Some datasets namespace their keys (EPalWazaID::AcidRain). The prefix is
// constant noise in a URL, so it is dropped before slugging.
export function stripKeyPrefix(key: string): string {
	const index = key.lastIndexOf('::');
	return index === -1 ? key : key.slice(index + 2);
}

/**
 * Records that carry a `disabled` flag, either at the top level (raw JSON) or
 * nested under `details` (the wire shape). Lives here rather than alongside the
 * descriptors so build-time route modules can filter without importing the
 * runtime data stores.
 */
export function isDisabledRecord(record: unknown): boolean {
	if (!record || typeof record !== 'object') return false;
	const value = record as Record<string, unknown>;
	if (value.disabled === true) return true;
	const details = value.details;
	return (
		!!details &&
		typeof details === 'object' &&
		(details as Record<string, unknown>).disabled === true
	);
}
