const yakushimaCorrelations: Record<string, string> = {
	yakushimamonster001: 'unique_yakushimamonster001_slimepress_leaf',
	yakushimamonster001_blue: 'unique_yakushimamonster001_slimepress_water',
	yakushimamonster001_pink: 'unique_yakushimamonster001_slimepress_normal',
	yakushimamonster001_purple: 'unique_yakushimamonster001_slimepress_dark',
	yakushimamonster001_rainbow: 'unique_yakushimamonster001_slimepress_rainbow',
	yakushimamonster001_red: 'unique_yakushimamonster001_slimepress_fire',
	yakushimamonster002: 'unique_yakushimamonster002_swordcharge',
	yakushimamonster003: 'unique_yakushimamonster003_batcharge',
	yakushimamonster003_purple: 'unique_yakushimamonster003_batcharge',
	yakushimaboss001: 'unique_yakushima_'
};

function normalizeCharacterKey(id: string): string {
	if (id.includes('nightlady')) return id.replace('_dark', '');
	if (id.includes('kingbahamut')) return id.replace('_dragon', '');
	return id;
}

function getUniqueSkillSubstring(characterKey: string): string {
	const normalized = normalizeCharacterKey(characterKey.toLowerCase());
	return yakushimaCorrelations[normalized] ?? `unique_${normalized}`;
}

export function isSkillAvailableForCharacter(
	skillId: string,
	characterKey: string
): boolean {
	const skillLower = skillId.toLowerCase();
	const uniqueSubstring = getUniqueSkillSubstring(characterKey);

	if (skillLower.includes(uniqueSubstring)) {
		return true;
	}
	if (!skillLower.includes('unique_')) {
		return true;
	}
	return false;
}