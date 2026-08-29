import type { WikiCategory } from './wikiCategories';

export type WikiSearchEntry = {
	category: WikiCategory;
	key: string;
	name: string;
};

function rank(name: string, query: string): number {
	const n = name.toLowerCase();
	if (n === query) return 0;
	if (n.startsWith(query)) return 1;
	if (n.includes(query)) return 2;
	return -1;
}

export function searchWiki(
	query: string,
	entries: WikiSearchEntry[],
	limit = 20
): WikiSearchEntry[] {
	const q = query.trim().toLowerCase();
	if (!q) return [];
	const scored: { entry: WikiSearchEntry; score: number }[] = [];
	for (const entry of entries) {
		const score = rank(entry.name, q);
		if (score >= 0) scored.push({ entry, score });
	}
	scored.sort((a, b) => {
		if (a.score !== b.score) return a.score - b.score;
		if (a.entry.name.length !== b.entry.name.length) {
			return a.entry.name.length - b.entry.name.length;
		}
		return a.entry.name.localeCompare(b.entry.name);
	});
	return scored.slice(0, limit).map((s) => s.entry);
}
