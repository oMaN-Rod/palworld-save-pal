import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type ActiveSkill, type ActiveSkillDetails } from '$types';

class ActiveSkills {
	private loading = false;

	activeSkills: Record<string, ActiveSkill> = $state({});

	private async ensureActiveSkillsLoaded(): Promise<void> {
		if (Object.keys(this.activeSkills).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.activeSkills = await sendAndWait(MessageType.GET_ACTIVE_SKILLS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching active skills:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureActiveSkillsLoaded();
		}
	}

	async searchActiveSkills(search: string): Promise<ActiveSkill | undefined> {
		await this.ensureActiveSkillsLoaded();
		return this.getByKey(search) || this.getByName(search) || undefined;
	}

	private getByKey(key: string): ActiveSkill | undefined {
		return this.activeSkills[key];
	}

	private getByName(name: string): ActiveSkill | undefined {
		return Object.values(this.activeSkills).find(
			(skill) => skill.localized_name.toLowerCase() === name.toLowerCase()
		);
	}

	async getField(
		key: string,
		field: string
	): Promise<string | ActiveSkillDetails | number | string[] | undefined> {
		await this.ensureActiveSkillsLoaded();
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
		await this.ensureActiveSkillsLoaded();
		return Object.values(this.activeSkills).filter(
			(skill) => skill.details.type.toLowerCase() === type.toLowerCase()
		);
	}

	async getActiveSkills(): Promise<ActiveSkill[]> {
		await this.ensureActiveSkillsLoaded();
		return Object.values(this.activeSkills);
	}

	async reset(): Promise<void> {
		this.activeSkills = {};
		await this.ensureActiveSkillsLoaded();
	}
}

export const activeSkillsData = new ActiveSkills();
