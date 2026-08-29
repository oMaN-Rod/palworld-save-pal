import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type PassiveSkill } from '$types';
import { normalizeKeys } from '$utils';

class PassiveSkills {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	passiveSkills: Record<string, PassiveSkill> = $state({});

	private async ensurePassiveSkillsLoaded(): Promise<void> {
		if (Object.keys(this.passiveSkills).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.passiveSkills = await sendAndWait(MessageType.GET_PASSIVE_SKILLS);
				this.keyMap = normalizeKeys(Object.keys(this.passiveSkills));
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

	getByKey(key: string): PassiveSkill | undefined {
		try {
			return this.passiveSkills[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting passive skill data:', error);
		}
	}

	async reset(): Promise<void> {
		this.passiveSkills = {};
		await this.ensurePassiveSkillsLoaded();
	}
}

export const passiveSkillsData = new PassiveSkills();
