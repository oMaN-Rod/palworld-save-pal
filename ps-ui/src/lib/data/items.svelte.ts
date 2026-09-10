import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Item } from '$types';
import { normalizeKeys } from '$utils';

class Items {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	items: Record<string, Item> = $state({});

	private async ensureItemsLoaded(): Promise<void> {
		if (Object.keys(this.items).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.items = await sendAndWait(MessageType.GET_ITEMS);
				this.keyMap = normalizeKeys(Object.keys(this.items));
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching items:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureItemsLoaded();
		}
	}

	getByKey(key: string): Item | undefined {
		try {
			return this.items[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting item data:', error);
		}
	}

	async reset(): Promise<void> {
		this.items = {};
		await this.ensureItemsLoaded();
	}
}

export const itemsData = new Items();
