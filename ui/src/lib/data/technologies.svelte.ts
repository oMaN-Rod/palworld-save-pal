import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Technology } from '$types';
import { normalizeKeys } from '$utils';

class Technologies {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	technologies: Record<string, Technology> = $state({});

	private async ensureTechnologiesLoaded(): Promise<void> {
		if (Object.keys(this.technologies).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.technologies = await sendAndWait(MessageType.GET_TECHNOLOGIES);
				this.keyMap = normalizeKeys(Object.keys(this.technologies));
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching technologies:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureTechnologiesLoaded();
		}
	}

	getByKey(key: string): Technology | undefined {
		try {
			return this.technologies[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting technology data:', error);
		}
	}

	async reset(): Promise<void> {
		this.technologies = {};
		await this.ensureTechnologiesLoaded();
	}
}

export const technologiesData = new Technologies();
