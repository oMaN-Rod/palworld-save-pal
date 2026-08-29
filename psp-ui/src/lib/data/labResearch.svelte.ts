import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Technology as LabResearch } from '$types';
import { normalizeKeys } from '$utils';

class LabResearchData {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	research: Record<string, LabResearch> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.research).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.research = await sendAndWait(MessageType.GET_LAB_RESEARCH);
				this.keyMap = normalizeKeys(Object.keys(this.research));
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching lab research:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	getByKey(key: string): LabResearch | undefined {
		try {
			return this.research[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting lab research data:', error);
		}
	}

	async reset(): Promise<void> {
		this.research = {};
		await this.ensureLoaded();
	}
}

export const labResearchData = new LabResearchData();
