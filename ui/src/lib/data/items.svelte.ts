import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Item, type ItemDetails, type ItemInfo } from '$types';

class Items {
	private loading = false;

	items: Record<string, Item> = $state({});

	private async ensureItemsLoaded(): Promise<void> {
		if (Object.keys(this.items).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.items = await sendAndWait(MessageType.GET_ITEMS);
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

	async searchItems(search: string): Promise<Item | undefined> {
		await this.ensureItemsLoaded();
		return this.getByKey(search) || this.getByName(search) || undefined;
	}

	private getByKey(key: string): Item | undefined {
		return this.items[key];
	}

	private getByName(name: string): Item | undefined {
		return Object.values(this.items).find(
			(item) => item.info.localized_name.toLowerCase() === name.toLowerCase()
		);
	}

	async getField(
		key: string,
		field: keyof Item | keyof ItemDetails | keyof ItemInfo
	): Promise<any> {
		await this.ensureItemsLoaded();
		const item = await this.searchItems(key);
		if (item) {
			if (field in item) {
				return item[field as keyof Item];
			} else if (field in item.details) {
				return item.details[field as keyof ItemDetails];
			} else if (field in item.info) {
				return item.info[field as keyof ItemInfo];
			}
		}
		return undefined;
	}

	async getAllItems(): Promise<Item[]> {
		await this.ensureItemsLoaded();
		return Object.values(this.items);
	}

	async getItemCount(): Promise<number> {
		await this.ensureItemsLoaded();
		return Object.keys(this.items).length;
	}

	async reset(): Promise<void> {
		this.items = {};
		await this.ensureItemsLoaded();
	}
}

export const itemsData = new Items();
