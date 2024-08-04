// src/lib/data/Elements.ts

import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$lib/utils/asset-loader';
import type { Element } from '$types';

class Elements {
	private elementsDict: Record<string, Element> = {};

	constructor() {
		this.initializeElements();
	}

	private async initializeElements() {
		const elementsData = await assetLoader.loadJson<Record<string, Element>>(
			`${ASSET_DATA_PATH}/data/elements.json`
		);
		this.elementsDict = Object.entries(elementsData).reduce(
			(acc, [key, value]) => {
				acc[key.toLowerCase()] = value;
				return acc;
			},
			{} as Record<string, Element>
		);
	}

	async searchElement(key: string): Promise<Element | undefined> {
		await this.ensureInitialized();
		return this.elementsDict[key.toLowerCase()];
	}

	async getField(key: string, field: keyof Element): Promise<string | undefined> {
		const element = await this.searchElement(key);
		return element ? element[field] : undefined;
	}

	async getAllElementTypes(): Promise<string[]> {
		await this.ensureInitialized();
		return Object.keys(this.elementsDict);
	}

	async getAllElements(): Promise<Element[]> {
		await this.ensureInitialized();
		return Object.values(this.elementsDict);
	}

	private async ensureInitialized() {
		if (Object.keys(this.elementsDict).length === 0) {
			await this.initializeElements();
		}
	}
}

export const elementsData = new Elements();
