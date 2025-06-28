import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Technology as LabResearch } from '$types';

class LabResearchData {
	private loading = false;

	research: Record<string, LabResearch> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.research).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.research = await sendAndWait(MessageType.GET_LAB_RESEARCH);
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

	async searchResearch(search: string): Promise<LabResearch | undefined> {
		await this.ensureLoaded();
		return this.getByKey(search) || this.getByName(search) || undefined;
	}

	private getByKey(key: string): LabResearch | undefined {
		return this.research[key];
	}

	private getByName(name: string): LabResearch | undefined {
		return Object.values(this.research).find(
			(r) => r.localized_name.toLowerCase() === name.toLowerCase()
		);
	}

	async getAllResearch(): Promise<LabResearch[]> {
		await this.ensureLoaded();
		return Object.values(this.research);
	}

	async getResearchByCategory(category: string): Promise<Record<string, LabResearch>> {
		await this.ensureLoaded();
		return Object.fromEntries(
			Object.entries(this.research).filter(([, r]) => r.details.category === category)
		);
	}

	async reset(): Promise<void> {
		this.research = {};
		await this.ensureLoaded();
	}
}

export const labResearchData = new LabResearchData();
