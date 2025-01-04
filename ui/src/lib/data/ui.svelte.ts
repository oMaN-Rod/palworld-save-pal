import { getSocketState } from '$states/websocketState.svelte';
import { MessageType, type UICommon } from '$types';

const DEFAULT_UI_COMMON: UICommon = {
	accessory: 'Accessory',
	active_skills: 'Active Skills',
	all: 'All',
	attack: 'Attack',
	body: 'Body',
	clear: 'Clear',
	defense: 'Defense',
	edit: 'Edit',
	element: 'Element',
	filter: 'Filter',
	food: 'Food',
	glider: 'Glider',
	head: 'Head',
	health: 'Health',
	inventory: 'Inventory',
	key_items: 'Key Items',
	level: 'LEVEL',
	max: 'MAX',
	next: 'NEXT',
	passive_skills: 'Passive Skills',
	san: 'SAN',
	search: 'Search',
	shield: 'Shield',
	sort: 'Sort',
	stamina: 'Stamina',
	stats: 'Stats',
	weapon: 'Weapon',
	weight: 'Weight',
	work_speed: 'Work Speed',
	work_suitability: 'Work Suitability'
};

export class UICommonData {
	private ws = getSocketState();
	private loading = false;

	strings: UICommon = $state(DEFAULT_UI_COMMON);

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.strings).length === 0 && !this.loading) {
			try {
				this.loading = true;
				const response = await this.ws.sendAndWait({
					type: MessageType.GET_UI_COMMON
				});
				if (response.type === 'error') {
					throw new Error(response.data);
				}
				this.strings = response.data;
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

	async reset(): Promise<void> {
		this.strings = DEFAULT_UI_COMMON;
		await this.ensureLoaded();
	}
}

export const uiCommonData = new UICommonData();
