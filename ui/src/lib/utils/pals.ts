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

export function canBeBoss(character_id: string): [string, boolean] {
	let valid = true;
	let type = '';
	if (character_id.toLowerCase().includes('predator_')) {
		valid = false;
		type = 'Predator';
	}
	if (character_id.toLowerCase().includes('summon_')) {
		valid = false;
		type = 'Summon';
	}
	if (character_id.toLowerCase().includes('raid_')) {
		valid = false;
		type = 'Raid';
	}
	return [type, valid];
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
	pal.level = appState.settings.cheat_mode ? 255 : 60;
	const maxLevelData = expData.expData['61'];
	pal.exp = maxLevelData.PalTotalEXP - maxLevelData.PalNextEXP;
	const [_, valid] = canBeBoss(pal.character_id);
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
}

export const applyPresetToPal = (pal: Record<string, any>, presetProfile: PresetProfile) => {
	const appState = getAppState();
	const palData = palsData.pals[pal.character_key];
	const skipKeys = ['character_id', 'lock', 'lock_element', 'element'];
	for (const [key, value] of Object.entries(presetProfile.pal_preset!)) {
		if (skipKeys.includes(key)) continue;
		if (key === 'is_boss') {
			if (!palData.is_pal) continue;
			pal.is_boss = value;
			pal.is_lucky = value ? false : pal.is_lucky;
		}
		if (key === 'is_lucky') {
			if (!palData.is_pal) continue;
			pal.is_boss = value ? false : pal.is_boss;
			pal.is_lucky = value;
		} else if (value != null) {
			(pal as Record<string, any>)[key] = value;
		}
	}
	getStats(pal as Pal, appState.selectedPlayer!);
	pal.hp = pal.max_hp;
	pal.stomach = palData.max_full_stomach;
	pal.state = EntryState.MODIFIED;
};
