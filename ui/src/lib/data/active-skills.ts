// src/lib/data/active-skills.ts

import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$lib/utils/asset-loader';
import type { ActiveSkill, ActiveSkillDetails } from '$types';

export class ActiveSkills {
	private active_skills: Record<string, ActiveSkill> = {};

	constructor() {
		this.initializeActiveSkills();
	}

	private async initializeActiveSkills() {
		const skillsData = await assetLoader.loadJson<Record<string, ActiveSkillDetails>>(
			`${ASSET_DATA_PATH}/data/active_skills.json`
		);
		const i18nData = await assetLoader.loadJson<Record<string, { name: string; description: string }>>(
			`${ASSET_DATA_PATH}/data/en-GB/active_skills.json`
		);

		for (const [skillId, details] of Object.entries(skillsData)) {
			const i18nInfo = i18nData[skillId] || { name: skillId, description: '' };
			const attack: ActiveSkill = {
				id: skillId,
				name: i18nInfo.name,
				details: {
					...details,
					description: i18nInfo.description
				}
			};
			this.active_skills[skillId.toLowerCase()] = attack;
		}
	}

	private formatActiveSkill(skill: string): string {
		return `epalwazaid::${skill}`;
	}

	async searchActiveSkills(search: string): Promise<ActiveSkill | null> {
		await this.ensureInitialized();
		const searchLower = `epalwazaid::${search}`.toLowerCase();
		return this.getByKey(searchLower) || this.getByName(searchLower) || null;
	}

	private getByKey(key: string): ActiveSkill | undefined {
		return this.active_skills[key.toLowerCase()];
	}

	private getByName(name: string): ActiveSkill | undefined {
		return Object.values(this.active_skills).find(
			(skill) => skill.name.toLowerCase() === name.toLowerCase()
		);
	}

	async getField(
		key: string,
		field: string
	): Promise<string | ActiveSkillDetails | number | string[] | undefined> {
		await this.ensureInitialized();
		const activeSkill = await this.searchActiveSkills(key);
		if (activeSkill) {
			if (field in activeSkill) {
				return activeSkill[field as keyof ActiveSkill];
			} else if (field in activeSkill.details) {
				return activeSkill.details[field as keyof ActiveSkillDetails];
			}
		}
		return undefined;
	}

	async searchActiveSkillsByType(type: string): Promise<ActiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.active_skills).filter(
			(skill) => skill.details.type.toLowerCase() === type.toLowerCase()
		);
	}

	async searchActiveSkillsByExclusive(exclusive: string): Promise<ActiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.active_skills).filter(
			(skill) => skill.details.exclusive && skill.details.exclusive.includes(exclusive)
		);
	}

	async getActiveSkills(): Promise<ActiveSkill[]> {
		await this.ensureInitialized();
		return Object.values(this.active_skills);
	}

	private async ensureInitialized() {
		if (Object.keys(this.active_skills).length === 0) {
			await this.initializeActiveSkills();
		}
	}
}

export const activeSkillsData = new ActiveSkills();