import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type PassiveSkill } from '$types';

class PassiveSkills {
	private loading = false;

	passiveSkills: Record<string, PassiveSkill> = $state({});

	private async ensurePassiveSkillsLoaded(): Promise<void> {
		if (Object.keys(this.passiveSkills).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.passiveSkills = await sendAndWait(MessageType.GET_PASSIVE_SKILLS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching passive skills:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensurePassiveSkillsLoaded();
		}
	}

	async searchPassiveSkills(search: string): Promise<PassiveSkill | undefined> {
		await this.ensurePassiveSkillsLoaded();
		return this.getByKey(search) || this.getByName(search) || undefined;
	}

	private getByKey(key: string): PassiveSkill | undefined {
		return this.passiveSkills[key];
	}

	private getByName(name: string): PassiveSkill | undefined {
		return Object.values(this.passiveSkills).find(
			(skill) => skill.localized_name.toLowerCase() === name.toLowerCase()
		);
	}

	async getPassiveSkills(): Promise<PassiveSkill[]> {
		await this.ensurePassiveSkillsLoaded();
		return Object.values(this.passiveSkills);
	}

	async searchPassiveSkillsByTier(tier: number): Promise<PassiveSkill[]> {
		await this.ensurePassiveSkillsLoaded();
		return Object.values(this.passiveSkills).filter((skill) => skill.details.rank === tier);
	}

	async reset(): Promise<void> {
		this.passiveSkills = {};
		await this.ensurePassiveSkillsLoaded();
	}
}

export const passiveSkillsData = new PassiveSkills();
