import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

export interface RelicRankData {
	cumulative_max: number;
	max_rank: number;
	per_rank: number[];
	/** Bonus granted at each rank; index 0 is rank 1. */
	effect_rate: number[];
	/** The game's own display name, e.g. "Satiety Duration" for hunger_reduction. */
	localized_name: string;
	description: string;
}

class RelicDataHandler {
	private loading: boolean = false;

	relicData: Record<string, RelicRankData> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.relicData).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.relicData = await sendAndWait(MessageType.GET_RELIC_DATA);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching relic data:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getRelicData(): Promise<Record<string, RelicRankData>> {
		await this.ensureLoaded();
		return this.relicData;
	}

	async reset(): Promise<void> {
		this.relicData = {};
		await this.ensureLoaded();
	}
}

export const relicData = new RelicDataHandler();
