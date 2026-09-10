import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Relic } from '$types';

export class Relics {
	private loading = false;

	points: Record<string, Relic> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.points).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.points = await sendAndWait(MessageType.GET_RELICS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching relics:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getRelics(): Promise<Record<string, Relic>> {
		await this.ensureLoaded();
		return this.points;
	}

	async reset(): Promise<void> {
		this.points = {};
		await this.ensureLoaded();
	}
}

export const relics = new Relics();
