/** `psp_plugin::manifest::MAX_ID_LEN` — the server refuses anything longer. */
const MAX_ID_LENGTH = 64;

export function slugify(name: string): string {
	const slug = name
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
	if (slug.length <= MAX_ID_LENGTH) return slug;
	return slug.slice(0, MAX_ID_LENGTH).replace(/-+$/, '');
}
