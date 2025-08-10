import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type ActiveSkill } from '$types';
import { normalizeKeys } from '$utils';

class ActiveSkills {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	activeSkills: Record<string, ActiveSkill> = $state({});

	private async ensureActiveSkillsLoaded(): Promise<void> {
		if (Object.keys(this.activeSkills).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.activeSkills = await sendAndWait(MessageType.GET_ACTIVE_SKILLS);
				this.keyMap = normalizeKeys(Object.keys(this.activeSkills));
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

	getByKey(key: string): ActiveSkill | undefined {
		try {
			return this.activeSkills[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting active skill by key:', error);
		}
	}

	async reset(): Promise<void> {
		this.activeSkills = {};
		await this.ensureActiveSkillsLoaded();
	}
}

export const activeSkillsData = new ActiveSkills();
