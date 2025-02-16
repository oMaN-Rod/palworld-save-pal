// src/lib/data/elements.ts

import { getSocketState } from '$states';
import { MessageType, type Building } from '$types';

export class Buildings {
	private ws = getSocketState();
	private loading = false;

	buildings: Record<string, Building> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.buildings).length === 0 && !this.loading) {
			try {
				this.loading = true;
				const response = await this.ws.sendAndWait({
					type: MessageType.GET_BUILDINGS
				});
				if (response.type === 'error') {
					throw new Error(response.data);
				}
				this.buildings = response.data;
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
