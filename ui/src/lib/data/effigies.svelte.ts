import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Effigy } from '$types';

export class Effigies {
	private loading = false;

	points: Record<string, Effigy> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.points).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.points = await sendAndWait(MessageType.GET_EFFIGIES);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching effigies:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getEffigies(): Promise<Record<string, Effigy>> {
		await this.ensureLoaded();
		return this.points;
	}

	async reset(): Promise<void> {
		this.points = {};
		await this.ensureLoaded();
	}
}

export const effigies = new Effigies();
