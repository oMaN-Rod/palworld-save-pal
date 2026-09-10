import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Element } from '$types';
import { normalizeKeys } from '$utils';

class Elements {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	elements: Record<string, Element> = $state({});

	private async ensureElementsLoaded(): Promise<void> {
		if (Object.keys(this.elements).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.elements = await sendAndWait(MessageType.GET_ELEMENTS);
				this.keyMap = normalizeKeys(Object.keys(this.elements));
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching elements:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureElementsLoaded();
		}
	}

	getByKey(key: string): Element | undefined {
		try {
			return this.elements[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting element data:', error);
		}
	}

	async reset(): Promise<void> {
		this.elements = {};
		await this.ensureElementsLoaded();
	}
}

export const elementsData = new Elements();
