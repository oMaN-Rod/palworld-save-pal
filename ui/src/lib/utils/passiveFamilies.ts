import type { PassiveSkill } from '$types';

export type PassiveMember = { key: string; skill: PassiveSkill };

export type PassiveFamily = {
	key: string;
	displayName: string;
	members: PassiveMember[];
	ranks: number[];
	primaryRank: number;
};

export function stripRankSuffix(name: string): string {
	return name
		.replace(/\s*Lv\.?\s*\d+\s*$/i, '')
		.replace(/\s*\+\d+\s*$/, '')
		.replace(/\s*\(\d+\)\s*$/, '')
		.replace(/\s+\d+\s*$/, '')
		.trim();
}

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
			primaryRank: positiveRanks.length > 0 ? Math.max(...positiveRanks) : members[0].skill.details.rank
		});
	}

	families.sort((a, b) => {
		if (b.primaryRank !== a.primaryRank) return b.primaryRank - a.primaryRank;
		return a.displayName.localeCompare(b.displayName);
	});

	return families;
}
