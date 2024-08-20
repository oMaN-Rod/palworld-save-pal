// src/lib/data/presets.ts

import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$lib/utils/asset-loader';
import type { ContainerSlot, ItemContainer } from '$types';

export interface PresetProfile {
    [key: string]: {
        [key: string]: ContainerSlot[];
    };
}

export class Presets {
    private presets: PresetProfile = {};

    constructor() {
        this.initializePresets();
    }

    private async initializePresets() {
        this.presets = await assetLoader.loadJson<PresetProfile>(`${ASSET_DATA_PATH}/data/presets.json`);
    }

    async getPresetProfiles(): Promise<string[]> {
        await this.ensureInitialized();
        return Object.keys(this.presets);
    }

    async getPresetProfile(profileName: string): Promise<PresetProfile[string] | undefined> {
        await this.ensureInitialized();
        return this.presets[profileName];
    }

    async applyPreset(profileName: string, containers: Record<string, ItemContainer>): Promise<Record<string, ItemContainer>> {
        const profile = await this.getPresetProfile(profileName);
        if (!profile) {
            throw new Error(`Preset profile "${profileName}" not found`);
        }

        const updatedContainers: Record<string, ItemContainer> = {};

        for (const [containerName, container] of Object.entries(containers)) {
            const presetSlots = profile[containerName] || containers[containerName].slots;
            const updatedSlots = container.slots.map((slot) => {
                const presetSlot = presetSlots.find(ps => ps.slot_index === slot.slot_index);
                if (presetSlot) {
                    return { ...slot, ...presetSlot };
                }
                return { ...slot, static_id: 'None', count: 0, dynamic_item: undefined };
            });

            updatedContainers[containerName] = { ...container, slots: updatedSlots };
        }

        return updatedContainers;
    }

    private async ensureInitialized() {
        if (Object.keys(this.presets).length === 0) {
            await this.initializePresets();
        }
    }
}

export const presetsData = new Presets();