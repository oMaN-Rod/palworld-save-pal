// src/lib/data/items.ts

import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$lib/utils/asset-loader';
import type { Item, ItemDetails, ItemInfo } from '$types';

export class Items {
    private items: Record<string, Item> = {};

    constructor() {
        this.initializeItems();
    }

    private async initializeItems() {
        const itemsData = await assetLoader.loadJson<Record<string, ItemDetails>>(
            `${ASSET_DATA_PATH}/data/items.json`
        );
        const itemInfoData = await assetLoader.loadJson<Record<string, ItemInfo>>(
            `${ASSET_DATA_PATH}/data/en-GB/items.json`
        );

        for (const [itemId, details] of Object.entries(itemsData)) {
            const info = itemInfoData[itemId] || { localized_name: itemId, description: "" };
            const item: Item = {
                id: itemId,
                details: details,
                info: info
            };
            this.items[itemId.toLowerCase()] = item;
        }
    }

    async searchItems(search: string): Promise<Item | null> {
        await this.ensureInitialized();
        const searchLower = search.toLowerCase();
        return this.getByKey(searchLower) || this.getByName(searchLower) || null;
    }

    private getByKey(key: string): Item | undefined {
        return this.items[key.toLowerCase()];
    }

    private getByName(name: string): Item | undefined {
        return Object.values(this.items).find(
            (item) => item.info.localized_name.toLowerCase() === name.toLowerCase()
        );
    }

    async getField(key: string, field: keyof Item | keyof ItemDetails | keyof ItemInfo): Promise<any> {
        await this.ensureInitialized();
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
        await this.ensureInitialized();
        return Object.values(this.items);
    }

    async getItemCount(): Promise<number> {
        await this.ensureInitialized();
        return Object.keys(this.items).length;
    }

    private async ensureInitialized() {
        if (Object.keys(this.items).length === 0) {
            await this.initializeItems();
        }
    }
}

export const itemsData = new Items();