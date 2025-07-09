import { friendshipData, palsData, passiveSkillsData } from '$lib/data';
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
		const skillData = passiveSkillsData.passiveSkills[skillId];
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

	const palData = palsData.pals[pal.character_key];
	if (!palData || !palData.is_pal || palData.is_tower_boss || palData.is_raid_boss) {
		return;
	}

	const level = Math.min(player.level, pal.level);

	// --- Multiplicative Bonuses ---
	const { attackBonus, defenseBonus, workSpeedBonus } = calculateSkillEffects(pal.passive_skills);
	const condenserBonus = (pal.rank - 1) * 0.05;
	const hpIvPercent = (pal.talent_hp * 0.3) / 100;
	const attackIvPercent = (pal.talent_shot * 0.3) / 100;
	const defenseIvPercent = (pal.talent_defense * 0.3) / 100;
	const hpSoulBonus = pal.rank_hp * 0.03;
	const attackSoulBonus = pal.rank_attack * 0.03;
	const defenseSoulBonus = pal.rank_defense * 0.03;

	// --- Additive Trust Bonus ---
	const trustLevels = Object.values(friendshipData.friendshipData).sort((a, b) => b.rank - a.rank);
	const trustLevel =
		trustLevels.find((l) => (pal.friendship_point ?? 0) >= l.required_point)?.rank ?? 0;

	const fAtkScale = palData.scaling.attack;
	const fHpScale = palData.scaling.hp;
	const fDefScale = palData.scaling.defense;

	// Scale IV from 0-100 to 0-1 for the trust formula
	const ivAtk = pal.talent_shot / 100.0;
	const ivHP = pal.talent_hp / 100.0;
	const ivDef = pal.talent_defense / 100.0;

	const trustAtk = fAtkScale * level * trustLevel * 0.1 * (0.75 + 0.25 * ivAtk);
	const trustHP = fHpScale * level * trustLevel * 0.65 * (0.75 + 0.25 * ivHP);
	const trustDef = fDefScale * level * trustLevel * 0.1 * (0.75 + 0.25 * ivDef);

	// --- Final Stat Calculations ---

	// HP calculation
	const alphaScaling = pal.is_boss || pal.is_lucky ? 1.2 : 1;
	const baseHp = Math.floor(
		500 + 5 * level + fHpScale * 0.5 * level * (1 + hpIvPercent) * alphaScaling
	);
	const multipliedHp = Math.floor(baseHp * (1 + condenserBonus) * (1 + hpSoulBonus));
	pal.max_hp = Math.floor(multipliedHp + trustHP) * 1000;

	// Attack calculation
	const baseAttack = Math.floor(fAtkScale * 0.075 * level * (1 + attackIvPercent));
	const multipliedAttack = Math.floor(
		baseAttack * (1 + condenserBonus) * (1 + attackSoulBonus) * (1 + attackBonus)
	);
	const attack = Math.floor(multipliedAttack + trustAtk);

	// Defense calculation
	const baseDefense = Math.floor(50 + fDefScale * 0.075 * level * (1 + defenseIvPercent));
	const multipliedDefense = Math.floor(
		baseDefense * (1 + condenserBonus) * (1 + defenseSoulBonus) * (1 + defenseBonus)
	);
	const defense = Math.floor(multipliedDefense + trustDef);

	// Work speed calculation (Trust doesn't affect this)
	const workSpeed = 70 * (1 + workSpeedBonus);

	return {
		attack,
		defense,
		workSpeed
	};
}
