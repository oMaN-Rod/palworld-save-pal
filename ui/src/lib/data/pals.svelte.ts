import { sendAndWait } from '$lib/utils/websocketUtils';
import { type PalData, MessageType } from '$types';

class Pals {
	private loading = false;

	pals: Record<string, PalData> = $state({});
	keyMap: Record<string, string> = $state({});

	private async ensurePalsLoaded(): Promise<void> {
		if (Object.keys(this.pals).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.pals = await sendAndWait(MessageType.GET_PALS);
				for (const key of Object.keys(this.pals)) {
					this.keyMap[key.toLowerCase()] = key;
				}
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching pals:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensurePalsLoaded();
		}
	}

	getPalData(key: string): PalData | undefined {
		try {
			return this.pals[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting pal data:', error);
		}
	}

	async searchByLocalizedName(localizedName: string): Promise<PalData | undefined> {
		await this.ensurePalsLoaded();
		return Object.values(this.pals).find(
			(pal) => pal.localized_name.toLowerCase() === localizedName.toLowerCase()
		);
	}

	async getAllPals(): Promise<[string, PalData][]> {
		await this.ensurePalsLoaded();
		return Object.entries(this.pals);
	}

	async reset(): Promise<void> {
		this.pals = {};
		await this.ensurePalsLoaded();
	}
}

export const palsData = new Pals();
