import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type FastTravelPoint } from '$types';

export class FastTravelPoints {
	private loading = false;

	points: Record<string, FastTravelPoint> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.points).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.points = await sendAndWait(MessageType.GET_FAST_TRAVEL_POINTS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching fast travel points:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getFastTravelPoints(): Promise<Record<string, FastTravelPoint>> {
		await this.ensureLoaded();
		return this.points;
	}

	async reset(): Promise<void> {
		this.points = {};
		await this.ensureLoaded();
	}
}

export const fastTravelPoints = new FastTravelPoints();
