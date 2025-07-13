import { expData, palsData } from '$lib/data';
import { getStats } from '$lib/utils';
import { getAppState } from '$states';
import {
	EntryState,
	type Pal,
	type Player,
	type PresetProfile,
	type WorkSuitability
} from '$types';

const prefixTypeMap = [
	['predator_', 'Predator'],
	['summon_', 'Summon'],
	['raid_', 'Raid'],
	['gym_', 'Tower Boss']
] as const;

export function canBeLucky(character_id: string): [string, boolean] {
	const lowerCaseId = character_id.toLowerCase();
	for (const [prefix, type] of prefixTypeMap) {
		if (lowerCaseId.includes(prefix)) {
			return [type, false];
		}
	}

	return ['', true];
}

export function canBeAlpha(character_id: string): [string, boolean] {
	const lowerCaseId = character_id.toLowerCase();
	const excludedPrefixes = [
		...prefixTypeMap,
		['yakushimamonster', 'This'],
		['yakushimaboss001_small', 'This'],
		['quest_farmer03_', 'This']
	];

	for (const [prefix, type] of excludedPrefixes) {
		if (lowerCaseId.startsWith(prefix)) {
			return [type, false];
		}
	}

	return ['', true];
}

export function formatNickname(nickname: string, prefix: string | undefined) {
	if (prefix && !nickname.startsWith(prefix)) {
		return `${prefix} ${nickname}`;
	}
	return nickname;
}

export async function handleMaxOutPal(pal: Pal, player: Player): Promise<void> {
	if (!pal || !player) return;
	const appState = getAppState();
	pal.level = appState.settings.cheat_mode ? 255 : 65;
	const maxLevelData = expData.expData['66'];
	pal.exp = maxLevelData.PalTotalEXP - maxLevelData.PalNextEXP;
	const [_, valid] = canBeLucky(pal.character_id);
	pal.is_boss = valid;
	pal.is_lucky = false;
	pal.talent_hp = appState.settings.cheat_mode ? 255 : 100;
	pal.talent_shot = appState.settings.cheat_mode ? 255 : 100;
	pal.talent_defense = appState.settings.cheat_mode ? 255 : 100;
	pal.rank = appState.settings.cheat_mode ? 255 : 5;
	pal.rank_hp = appState.settings.cheat_mode ? 255 : 20;
	pal.rank_defense = appState.settings.cheat_mode ? 255 : 20;
	pal.rank_attack = appState.settings.cheat_mode ? 255 : 20;
	pal.rank_craftspeed = appState.settings.cheat_mode ? 255 : 20;
	getStats(pal, player);
	pal.hp = pal.max_hp;
	pal.state = EntryState.MODIFIED;
	const palData = palsData.pals[pal.character_key];
	if (palData) {
		pal.stomach = palData.max_full_stomach;
		for (const [key, value] of Object.entries(palData.work_suitability)) {
			if (value === 0) continue;
			pal.work_suitability[key as WorkSuitability] = Math.min(5 - value, 4);
		}
	} else {
		pal.stomach = 150;
	}
	pal.friendship_point = 200000
}

export const applyPalPreset = (pal: Pal, presetProfile: PresetProfile, player: Player): void => {
	if (!presetProfile.pal_preset) return;

	const palData = palsData.pals[pal.character_key];
	if (!palData) return;

	const skipKeys = new Set(['character_id', 'character_key', 'lock', 'lock_element', 'element']);
	const [, canBeBossValue] = canBeLucky(pal.character_id);

	for (const [key, value] of Object.entries(presetProfile.pal_preset)) {
		if (skipKeys.has(key) || value == null) continue;

		if (key === 'is_boss' || key === 'is_lucky') {
			handleBossLuckyFlags(pal, palData, key, value as boolean, canBeBossValue);
		} else {
			(pal as Record<string, any>)[key] = value;
		}
	}

	getStats(pal, player);
	pal.hp = pal.max_hp;
	pal.stomach = palData.max_full_stomach;
	pal.state = EntryState.MODIFIED;
};

function handleBossLuckyFlags(
	pal: Pal,
	palData: any,
	key: 'is_boss' | 'is_lucky',
	value: boolean,
	canBeBossValue: boolean
): void {
	if (!canBeBossValue || !palData.is_pal) {
		pal.is_boss = false;
		pal.is_lucky = false;
		return;
	}

	if (key === 'is_boss') {
		pal.is_boss = value;
		if (value) pal.is_lucky = false;
	} else {
		pal.is_lucky = value;
		if (value) pal.is_boss = false;
	}
}
