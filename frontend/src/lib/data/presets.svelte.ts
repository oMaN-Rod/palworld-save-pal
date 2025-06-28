import { send, sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type PresetProfile } from '$types';

class Presets {
	private loading = false;

	presetProfiles: Record<string, PresetProfile> = $state({});

	private async ensurePresetProfilesLoaded(): Promise<void> {
		if (Object.keys(this.presetProfiles).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.presetProfiles = await sendAndWait(MessageType.GET_PRESETS);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching presets:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensurePresetProfilesLoaded();
		}
	}

	async getPresetProfiles(): Promise<Record<string, PresetProfile>> {
		await this.ensurePresetProfilesLoaded();
		return this.presetProfiles;
	}

	async addPresetProfile(profile: PresetProfile): Promise<Record<string, PresetProfile>> {
		try {
			await sendAndWait(MessageType.ADD_PRESET, profile);
			return await this.reset();
		} catch (error) {
			console.error('Error adding preset:', error);
			throw error;
		}
	}

	async changePresetName(id: string, name: string): Promise<Record<string, PresetProfile>> {
		console.log('changeProfileName', id, name);
		try {
			const profiles = await this.getPresetProfiles();
			const profile = profiles[id];
			if (profile) {
				profile.name = name;
				await send(MessageType.UPDATE_PRESET, {
					id: id,
					name: profile.name
				});
			}
			return await this.reset();
		} catch (error) {
			console.error('Error changing profile name:', error);
			throw error;
		}
	}

	async clone(id: string, name: string): Promise<Record<string, PresetProfile>> {
		try {
			const profiles = await this.getPresetProfiles();
			const profile = profiles[id];
			if (profile) {
				const newProfile = { ...profile, name: name };
				await sendAndWait(MessageType.ADD_PRESET, newProfile);
			}
			return this.reset();
		} catch (error) {
			console.error('Error cloning profile:', error);
			throw error;
		}
	}

	async removePresetProfiles(ids: string[]): Promise<Record<string, PresetProfile>> {
		try {
			await sendAndWait(MessageType.DELETE_PRESET, ids);
			return await this.reset();
		} catch (error) {
			console.error('Error removing preset profiles:', error);
			throw error;
		}
	}

	async reset(): Promise<Record<string, PresetProfile>> {
		this.presetProfiles = {};
		return this.getPresetProfiles();
	}
}

export const presetsData = new Presets();
