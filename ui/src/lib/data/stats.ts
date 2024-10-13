import type { Pal, Player } from '$types';
import { palsData, passiveSkillsData } from '.';

export type PalStats = {
	attack: number;
	defense: number;
	work_speed?: number;
};

export async function getStats(pal: Pal, player: Player): Promise<PalStats | undefined> {
	if (!pal) {
		console.log('No pal provided');
		return;
	}
	if (!player) {
		console.log('No player provided');
		return;
	}
	const palData = await palsData.getPalInfo(pal.character_id);
	if (!palData) {
		console.log('No pal data found');
		return;
	}
	if (!palData.is_pal || palData.is_tower_boss || palData.is_raid_boss) {
		return;
	}

	const level = player.level < pal.level ? player.level : pal.level;
	let attackBonus = 0;
	let defenseBonus = 0;
	let workSpeedBonus = 0;
	for (let i = 0; i < pal.passive_skills.length; i++) {
		const skill = pal.passive_skills[i];
		const skillData = await passiveSkillsData.searchPassiveSkills(skill);
		if (!skillData) {
			continue;
		}
		attackBonus += skillData.details.bonuses.attack / 100;
		defenseBonus += skillData.details.bonuses.defense / 100;
		workSpeedBonus += skillData.details.bonuses.work_speed / 100;
	}
	const condenserBonus = (pal.rank - 1) * 0.05;
	const hpIv = (pal.talent_hp * 0.3) / 100;
	const hpSoulBonus = pal.rank_hp * 0.03;
	const hpScale = palData.scaling.hp;
	const hp = Math.floor(500 + 5 * level + hpScale * 0.5 * level * (1 + hpIv));
	const alphaScaling = pal.is_boss ? 1.2 : 1;
	pal.max_hp = Math.floor(hp * (1 + condenserBonus) * (1 + hpSoulBonus) * alphaScaling) * 1000;

	const attackIv = (pal.talent_shot * 0.3) / 100;
	const attackSoulBonus = pal.rank_attack * 0.03;
	const attackScale = palData.scaling.attack;

	let attack = Math.floor(attackScale * 0.075 * level * (1 + attackIv));
	attack = Math.floor(attack * (1 + condenserBonus) * (1 + attackSoulBonus) * (1 + attackBonus));

	const defenseIv = (pal.talent_defense * 0.3) / 100;
	const defenseSoulBonus = pal.rank_defense * 0.03;
	const defenseScale = palData.scaling.defense;

	let defense = Math.floor(50 + defenseScale * 0.075 * level * (1 + defenseIv));
	defense = Math.floor(
		defense * (1 + condenserBonus) * (1 + defenseSoulBonus) * (1 + defenseBonus)
	);

	return {
		attack,
		defense
	};
}
