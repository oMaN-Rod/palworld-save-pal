import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Building } from '$types';

class Buildings {
	private loading = false;

	buildings: Record<string, Building> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.buildings).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.buildings = await sendAndWait(MessageType.GET_BUILDINGS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching buildings:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async reset(): Promise<void> {
		this.buildings = {};
		await this.ensureLoaded();
	}
}

export const buildingsData = new Buildings();
