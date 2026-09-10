// Spawn-context markers, not species. Anything else leading a character_id is part
// of the species name and must not be touched.
const SPAWN_PREFIXES = ['boss_', 'predator_', 'summon_', 'raid_', 'gym_'];

export function resolvePalModelKey(
	rawKey: string,
	has: (key: string) => boolean
): string | null {
	const key = rawKey.toLowerCase();
	if (!key) return null;
	// First, so a model whose own name begins with a prefix is found as itself
	// rather than mistaken for a prefixed variant of something shorter.
	if (has(key)) return key;

	const prefix = SPAWN_PREFIXES.find((p) => key.startsWith(p) && key.length > p.length);
	// Only the remainder is peeled: peeling the prefixed form would offer the bare
	// prefix ("boss", "raid") as a candidate, which every model-less Pal collides on.
	let candidate = prefix ? key.slice(prefix.length) : key;

	for (;;) {
		if (has(candidate)) return candidate;
		const cut = candidate.lastIndexOf('_');
		if (cut < 0) return null;
		candidate = candidate.slice(0, cut);
	}
}
