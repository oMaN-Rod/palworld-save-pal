// src/lib/data/presets.ts

import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from '$lib/utils/asset-loader';
import { persistedState } from '$states/persistedState.svelte';
import type { ContainerSlot, ItemContainer } from '$types';

export interface PresetProfile {
	name: string;
	common_container?: ContainerSlot[];
	essential_container?: ContainerSlot[];
	weapon_load_out_container?: ContainerSlot[];
	player_equipment_armor_container?: ContainerSlot[];
	food_equip_container?: ContainerSlot[];
}

export class Presets {
	private presetsState;

	constructor() {
		this.presetsState = persistedState<PresetProfile[]>('palworld-presets', [], {
			onParseError: (error) => {
				console.error('Error parsing presets:', error);
				this.initializePresets();
			}
		});
		this.initializePresets();
	}

	private async initializePresets() {
		if (this.presetsState.value.length === 0) {
			const defaultPresets = await assetLoader.loadJson<PresetProfile[]>(
				`${ASSET_DATA_PATH}/data/presets.json`
			);
			this.presetsState.value = defaultPresets;
		}
	}

	async getPresetProfiles(): Promise<PresetProfile[]> {
		await this.ensureInitialized();
		return this.presetsState.value;
	}

	async getPresetProfilesNames(): Promise<string[]> {
		await this.ensureInitialized();
		return this.presetsState.value.map((profile) => profile.name);
	}

	async getPresetProfile(profileName: string): Promise<PresetProfile | undefined> {
		await this.ensureInitialized();
		return this.presetsState.value.find((profile) => profile.name === profileName);
	}

	async applyPreset(
		profileName: string,
		containers: Record<string, ItemContainer>
	): Promise<Record<string, ItemContainer>> {
		const profile = await this.getPresetProfile(profileName);
		if (!profile) {
			throw new Error(`Preset profile "${profileName}" not found`);
		}

		const updatedContainers: Record<string, ItemContainer> = {};

		for (const [containerName, container] of Object.entries(containers)) {
			const presetSlots = profile[containerName as keyof PresetProfile] || container.slots;
			const updatedSlots = container.slots.map((slot) => {
				const presetSlot = (presetSlots as ContainerSlot[])?.find(
					(ps) => ps.slot_index === slot.slot_index
				);
				if (presetSlot) {
					return { ...slot, ...presetSlot };
				}
				return { ...slot, static_id: 'None', count: 0, dynamic_item: undefined };
			});

			updatedContainers[containerName] = { ...container, slots: updatedSlots };
		}

		return updatedContainers;
	}

	async addPresetProfile(profile: PresetProfile) {
		await this.ensureInitialized();
		this.presetsState.value = [...this.presetsState.value, profile];
		return this.presetsState.value;
	}

	async changeProfileName(oldProfileName: string, newProfileName: string) {
		await this.ensureInitialized();
		const profile = this.presetsState.value.find((profile) => profile.name === oldProfileName);
		if (profile) {
			profile.name = newProfileName;
		}
		return this.presetsState.value;
	}

	async clone(profileName: string, newProfileName: string) {
		await this.ensureInitialized();
		const profile = this.presetsState.value.find((profile) => profile.name === profileName);
		if (profile) {
			const newProfile = { ...profile, name: newProfileName };
			this.presetsState.value = [...this.presetsState.value, newProfile];
		}
		return this.presetsState.value;
	}

	async removePresetProfiles(profileNames: string[]) {
		await this.ensureInitialized();
		this.presetsState.value = this.presetsState.value.filter(
			(profile) => !profileNames.includes(profile.name)
		);
		return this.presetsState.value;
	}

	private async ensureInitialized() {
		if (this.presetsState.value.length === 0) {
			await this.initializePresets();
		}
	}
}

export const presetsData = new Presets();
