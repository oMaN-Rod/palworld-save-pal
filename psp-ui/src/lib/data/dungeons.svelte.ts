import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Dungeon } from '$types';

export class Dungeons {
	private loading = false;

	points: Record<string, Dungeon> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.points).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.points = await sendAndWait(MessageType.GET_DUNGEONS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching dungeons:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getDungeons(): Promise<Record<string, Dungeon>> {
		await this.ensureLoaded();
		return this.points;
	}

	async reset(): Promise<void> {
		this.points = {};
		await this.ensureLoaded();
	}
}

export const dungeons = new Dungeons();
