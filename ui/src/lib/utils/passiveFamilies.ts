import type { PassiveSkill } from '$types';

export type PassiveMember = { key: string; skill: PassiveSkill };

export type PassiveFamily = {
	/** Stable key for selection state (the stripped display name). */
	key: string;
	/** Display name with rank/level tokens removed, e.g. "Attack Up". */
	displayName: string;
	/** Members sorted by rank then id. */
	members: PassiveMember[];
	/** Distinct positive ranks present, ascending. */
	ranks: number[];
	/** Highest rank in the family (used for list ordering/tier badge). */
	primaryRank: number;
};

/**
 * Strips trailing rank/level markers from a localized passive name.
 * Handles "Lv. N", "+N", "(N)", and trailing " N" suffixes.
 * "Attack Up Lv. 2" → "Attack Up"; "Aerial Dash +3" → "Aerial Dash".
 */
export function stripRankSuffix(name: string): string {
	return name
		.replace(/\s*Lv\.?\s*\d+\s*$/i, '')
		.replace(/\s*\+\d+\s*$/, '')
		.replace(/\s*\(\d+\)\s*$/, '')
		.replace(/\s+\d+\s*$/, '')
		.trim();
}

/**
 * Groups passive skills into families by their base localized name with rank
 * tokens removed. Skills whose stripped names match form one family and can be
 * compared across ranks (e.g. Attack Up Lv. 2/3/4). Skills with unique names
 * become single-member families.
 *
 * ponytail: name-heuristic grouping — entries with unrelated names that share
 * a code prefix (e.g. Deffence_up1/2/2_2/3 → "Hard Skin"/"Burly Body"/...)
 * correctly stay separate. Upgrade to a backend family field if exposed.
 */
export function groupPassiveFamilies(entries: [string, PassiveSkill][]): PassiveFamily[] {
	const byBaseName = new Map<string, PassiveMember[]>();

	for (const [key, skill] of entries) {
		const baseName = stripRankSuffix(skill.localized_name || key);
		const list = byBaseName.get(baseName);
		if (list) {
			list.push({ key, skill });
		} else {
			byBaseName.set(baseName, [{ key, skill }]);
		}
	}

	const families: PassiveFamily[] = [];
	for (const [baseName, members] of byBaseName) {
		members.sort((a, b) => {
			const rankDiff = a.skill.details.rank - b.skill.details.rank;
			return rankDiff !== 0 ? rankDiff : a.key.localeCompare(b.key);
		});
		const ranks = Array.from(
			new Set(members.map((m) => m.skill.details.rank).filter((r) => r > 0))
		).sort((a, b) => a - b);
		const positiveRanks = members.map((m) => m.skill.details.rank).filter((r) => r > 0);
		families.push({
			key: baseName,
			displayName: baseName,
			members,
			ranks,
			primaryRank:
				positiveRanks.length > 0 ? Math.max(...positiveRanks) : members[0].skill.details.rank
		});
	}

	families.sort((a, b) => {
		if (b.primaryRank !== a.primaryRank) return b.primaryRank - a.primaryRank;
		return a.displayName.localeCompare(b.displayName);
	});

	return families;
}
