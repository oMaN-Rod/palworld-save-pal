// src/lib/data/Pals.ts

import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$utils/asset-loader';
import type { Element, ElementType, Moveset, Scaling, Suitabilities, Bonuses, PalData } from '$types';

export class PalInfo {
	code_name: string;
	localized_name: string;
	elements: ElementType[];
	move_set: Moveset;
	raid_move_set?: Moveset;
	scaling: Scaling;
	suitabilities: Suitabilities;
	is_tower: boolean;
	is_human: boolean;
	bonuses?: Bonuses;

	constructor(code_name: string, localized_name: string, data: PalData) {
		this.code_name = code_name;
		this.localized_name = localized_name;
		this.elements = data.Type;
		this.move_set = data.Moveset;
		this.raid_move_set = data.RaidMoveset;
		this.scaling = data.Scaling;
		this.suitabilities = data.Suitabilities;
		this.is_tower = data.Tower || false;
		this.is_human = data.Human || false;
		this.bonuses = data.Bonuses;
	}
}

export class Pals {
	private pals: Record<string, PalInfo> = {};
	private elements: Record<string, Element> = {};

	constructor() {
		this.initializePals();
	}

	private async initializePals() {
		const palsData = await assetLoader.loadJson<{ values: PalData[] }>(
			`${ASSET_DATA_PATH}/data/pals.json`
		);
		const palNamesData = await assetLoader.loadJson<Record<string, string>>(
			`${ASSET_DATA_PATH}/data/en-GB/pals.json`
		);
		this.elements = await assetLoader.loadJson<Record<string, Element>>(
			`${ASSET_DATA_PATH}/data/elements.json`
		);

		palsData.values.forEach((pal: PalData) => {
			const codeName = pal.CodeName;
			const localizedName = palNamesData[codeName] || codeName;
			this.pals[codeName] = new PalInfo(codeName, localizedName, pal);
		});
	}

	async getPalInfo(key: string): Promise<PalInfo | undefined> {
		await this.ensureInitialized();
		return (
			this.pals[key] ||
			Object.values(this.pals).find((pal) => pal.localized_name.toLowerCase() === key.toLowerCase())
		);
	}

	async searchPals(search: string): Promise<PalInfo[]> {
		await this.ensureInitialized();
		const lowercaseSearch = search.toLowerCase();
		return Object.values(this.pals).filter(
			(pal) =>
				pal.code_name.toLowerCase().includes(lowercaseSearch) ||
				pal.localized_name.toLowerCase().includes(lowercaseSearch)
		);
	}

	async getPalsByType(palType: ElementType): Promise<PalInfo[]> {
		await this.ensureInitialized();
		return Object.values(this.pals).filter((pal) => pal.elements.includes(palType));
	}

	async getPalsBySuitability(suitability: string, minLevel: number = 1): Promise<PalInfo[]> {
		await this.ensureInitialized();
		return Object.values(this.pals).filter(
			(pal) => (pal.suitabilities[suitability] || 0) >= minLevel
		);
	}

	async getAllPals(): Promise<PalInfo[]> {
		await this.ensureInitialized();
		return Object.values(this.pals);
	}

	async getPalCount(): Promise<number> {
		await this.ensureInitialized();
		return Object.keys(this.pals).length;
	}

	async getAllTypes(): Promise<string[]> {
		await this.ensureInitialized();
		return Object.keys(this.elements);
	}

	async getAllSuitabilities(): Promise<string[]> {
		await this.ensureInitialized();
		return Array.from(
			new Set(Object.values(this.pals).flatMap((pal) => Object.keys(pal.suitabilities)))
		);
	}

	async getTowerPals(): Promise<PalInfo[]> {
		await this.ensureInitialized();
		return Object.values(this.pals).filter((pal) => pal.is_tower);
	}

	async getHumanPals(): Promise<PalInfo[]> {
		await this.ensureInitialized();
		return Object.values(this.pals).filter((pal) => pal.is_human);
	}

	async getElement(elementName: string): Promise<Element | undefined> {
		await this.ensureInitialized();
		return this.elements[elementName];
	}

	private async ensureInitialized() {
		if (Object.keys(this.pals).length === 0) {
			await this.initializePals();
		}
	}
}

export const palsData = new Pals();
