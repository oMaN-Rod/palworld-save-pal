import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Building } from '$types';
import { normalizeKeys } from '$utils';

class Buildings {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	buildings: Record<string, Building> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.buildings).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.buildings = await sendAndWait(MessageType.GET_BUILDINGS);
				this.keyMap = normalizeKeys(Object.keys(this.buildings));
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

	getByKey(key: string): Building | undefined {
		try {
			return this.buildings[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting building data:', error);
		}
	}

	async reset(): Promise<void> {
		this.buildings = {};
		await this.ensureLoaded();
	}
}

export const buildingsData = new Buildings();
