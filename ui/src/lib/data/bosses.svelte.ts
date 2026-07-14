import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Boss } from '$types';

export class Bosses {
	private loading = false;

	points: Record<string, Boss> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.points).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.points = await sendAndWait(MessageType.GET_BOSSES);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching bosses:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getBosses(): Promise<Record<string, Boss>> {
		await this.ensureLoaded();
		return this.points;
	}

	async reset(): Promise<void> {
		this.points = {};
		await this.ensureLoaded();
	}
}

export const bosses = new Bosses();
