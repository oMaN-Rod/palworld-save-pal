import { palsData, passiveSkillsData } from '$lib/data';
import type { Pal, Player } from '$types';
import { EffectType, TargetType } from '$types';

export type PalStats = {
	attack: number;
	defense: number;
	workSpeed?: number;
};

function isDefenseEffect(type: EffectType): boolean {
	return type === EffectType.Defense || type.toString().startsWith('ElementResist_');
}

function isAttackEffect(type: EffectType): boolean {
	return (
		type === EffectType.ShotAttack ||
		type.toString().startsWith('Element') ||
		type.toString().startsWith('ElementBoost_')
	);
}

function isWorkSpeedEffect(type: EffectType): boolean {
	return type === EffectType.CraftSpeed;
}

function calculateSkillEffects(skills: string[]): {
	attackBonus: number;
	defenseBonus: number;
	workSpeedBonus: number;
} {
	let attackBonus = 0;
	let defenseBonus = 0;
	let workSpeedBonus = 0;

	for (const skillId of skills) {
		const skillData = passiveSkillsData.getByKey(skillId);
		if (!skillData) continue;

		for (const effect of skillData.details.effects) {
			if (effect.target !== TargetType.ToSelf && effect.target !== TargetType.ToSelfAndTrainer) {
				continue;
			}

			const effectValue = effect.value / 100;

			if (isDefenseEffect(effect.type)) {
				defenseBonus += effectValue;
			} else if (isAttackEffect(effect.type)) {
				attackBonus += effectValue;
			} else if (isWorkSpeedEffect(effect.type)) {
				workSpeedBonus += effectValue;
			}
		}
	}

	return {
		attackBonus,
		defenseBonus,
		workSpeedBonus
	};
}

export function getStats(pal: Pal, player: Player): PalStats | undefined {
	if (!pal) {
		console.log('No pal provided');
		return;
	}
	if (!player) {
		console.log('No player provided');
		return;
	}

	const palData = palsData.getByKey(pal.character_key);
	if (!palData) {
		console.log(`No pal data found for ${pal.character_key}`);
		return;
	}
	if (!palData.is_pal || palData.is_tower_boss || palData.is_raid_boss) {
		return;
	}

	const level = Math.min(player.level, pal.level);

	// Calculate bonuses from passive skills
	const { attackBonus, defenseBonus, workSpeedBonus } = calculateSkillEffects(pal.passive_skills);

	// Soul and condenser bonuses
	const condenserBonus = (pal.rank - 1) * 0.05;
	const hpIv = (pal.talent_hp * 0.3) / 100;
	const hpSoulBonus = pal.rank_hp * 0.03;
	const hpScale = palData.scaling.hp;

	// HP calculation
	const alphaScaling = pal.is_boss || pal.is_lucky ? 1.2 : 1;
	const hp = Math.floor(500 + 5 * level + hpScale * 0.5 * level * (1 + hpIv) * alphaScaling);
	pal.max_hp = Math.floor(hp * (1 + condenserBonus) * (1 + hpSoulBonus)) * 1000;

	// Attack calculation
	const attackIv = (pal.talent_shot * 0.3) / 100;
	const attackSoulBonus = pal.rank_attack * 0.03;
	const attackScale = palData.scaling.attack;
	let attack = Math.floor(attackScale * 0.075 * level * (1 + attackIv));
	attack = Math.floor(attack * (1 + condenserBonus) * (1 + attackSoulBonus) * (1 + attackBonus));

	// Defense calculation
	const defenseIv = (pal.talent_defense * 0.3) / 100;
	const defenseSoulBonus = pal.rank_defense * 0.03;
	const defenseScale = palData.scaling.defense;
	let defense = Math.floor(50 + defenseScale * 0.075 * level * (1 + defenseIv));
	defense = Math.floor(
		defense * (1 + condenserBonus) * (1 + defenseSoulBonus) * (1 + defenseBonus)
	);

	// Work speed calculation with base value of 70
	let workSpeed = 70 * (1 + workSpeedBonus);

	return {
		attack,
		defense,
		workSpeed
	};
}
