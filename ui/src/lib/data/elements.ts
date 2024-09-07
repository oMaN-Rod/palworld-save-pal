// src/lib/data/elements.ts

import { getSocketState } from '$states/websocketState.svelte';
import { MessageType, type Element } from '$types';

export class Elements {
    private ws = getSocketState();
    private elements: Record<string, Element> = {};

    private async ensureElementsLoaded(): Promise<void> {
        if (Object.keys(this.elements).length === 0) {
            try {
                const response = await this.ws.sendAndWait({ 
                    type: MessageType.GET_ELEMENTS 
                });
                if (response.type === 'error') {
                    throw new Error(response.data);
                }
                this.elements = response.data;
            } catch (error) {
                console.error('Error fetching elements:', error);
                throw error;
            }
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
}

export const elementsData = new Elements();