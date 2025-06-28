import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Element } from '$types';

class Elements {
	private loading = false;

	elements: Record<string, Element> = $state({});

	private async ensureElementsLoaded(): Promise<void> {
		if (Object.keys(this.elements).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.elements = await sendAndWait(MessageType.GET_ELEMENTS);
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

	async searchElement(key: string): Promise<Element | undefined> {
		await this.ensureElementsLoaded();
		return this.elements[key];
	}

	async getField(key: string, field: keyof Element): Promise<string | undefined> {
		const element = await this.searchElement(key);
		return element ? element[field] : undefined;
	}

	async getAllElementTypes(): Promise<string[]> {
		await this.ensureElementsLoaded();
		return Object.keys(this.elements);
	}

	async getAllElements(): Promise<Element[]> {
		await this.ensureElementsLoaded();
		return Object.values(this.elements);
	}

	async reset(): Promise<void> {
		this.elements = {};
		await this.ensureElementsLoaded();
	}
}

export const elementsData = new Elements();
