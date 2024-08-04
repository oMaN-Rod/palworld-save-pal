import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$lib/utils/asset-loader';
import type { PassiveSkill, PassiveSkillDetails, Bonuses } from '$types';

export class PassiveSkills {
	private passive_skills: Record<string, PassiveSkill> = {};

	constructor() {
		this.initializePassiveSkills();
	}

	private async initializePassiveSkills() {
		const i18nData = await assetLoader.loadJson<
			Record<string, { Name: string; Description: string; Effect: string }>
		>(`${ASSET_DATA_PATH}/data/en-GB/passive_skills.json`);
		const statsData = await assetLoader.loadJson<
			Record<string, { Rating?: string; Tier?: string; Bonuses?: Bonuses }>
		>(`${ASSET_DATA_PATH}/data/passive_skills.json`);

		for (const [skillId, details] of Object.entries(i18nData)) {
			const stats = statsData[skillId] || {};
			const skill: PassiveSkill = {
				id: skillId,
				name: details.Name,
				details: {
					Description: details.Description,
					Effect: details.Effect,
					Rating: stats.Rating || '',
					Tier: stats.Tier || '',
					Bonuses: stats.Bonuses || { Attack: 0, Defense: 0, WorkSpeed: 0 }
				}
			};
			this.passive_skills[skillId.toLowerCase()] = skill;
		}
	}

	async searchPassiveSkills(search: string): Promise<PassiveSkill | null> {
		await this.ensureInitialized();
		const lowerSearch = search.toLowerCase();
		return this.getByKey(lowerSearch) || this.getByName(lowerSearch) || null;
	}

	private getByKey(key: string): PassiveSkill | undefined {
		return this.passive_skills[key.toLowerCase()];
	}

	private getByName(name: string): PassiveSkill | undefined {
		return Object.values(this.passive_skills).find(
			(skill) => skill.name.toLowerCase() === name.toLowerCase()
		);
	}

	async getPassiveSkills(): Promise<PassiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.passive_skills);
	}

	async getField(
		key: string,
		field: keyof PassiveSkill | keyof PassiveSkillDetails
	): Promise<string | null> {
		const passiveSkill = await this.searchPassiveSkills(key);
		if (passiveSkill) {
			if (field in passiveSkill) {
				return passiveSkill[field as keyof PassiveSkill] as string;
			} else if (field in passiveSkill.details) {
				return passiveSkill.details[field as keyof PassiveSkillDetails];
			}
		}
		return null;
	}

	async searchPassiveSkillsByEffect(effect: string): Promise<PassiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.passive_skills).filter(
			(skill) => skill.details.Effect.toLowerCase() === effect.toLowerCase()
		);
	}

	async searchPassiveSkillsByTier(tier: string): Promise<PassiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.passive_skills).filter(
			(skill) => skill.details.Tier.toLowerCase() === tier.toLowerCase()
		);
	}

	async searchPassiveSkillsByRating(rating: string): Promise<PassiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.passive_skills).filter(
			(skill) => skill.details.Rating.toLowerCase() === rating.toLowerCase()
		);
	}

	private async ensureInitialized() {
		if (Object.keys(this.passive_skills).length === 0) {
			await this.initializePassiveSkills();
		}
	}
}

export const passiveSkillsData = new PassiveSkills();
