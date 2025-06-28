import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type MapObject } from '$types';

export class MapObjects {
	private loading = false;

	points: MapObject[] = $state([]);

	private async ensureLoaded(): Promise<void> {
		if (this.points.length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.points = await sendAndWait(MessageType.GET_MAP_OBJECTS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching active skills:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getMapObjects(): Promise<MapObject[]> {
		await this.ensureLoaded();
		return this.points;
	}

	async reset(): Promise<void> {
		this.points = [];
		await this.ensureLoaded();
	}
}

export const mapObjects = new MapObjects();
