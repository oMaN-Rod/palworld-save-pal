import { getSocketState } from '$states/websocketState.svelte';
import { MessageType, type PresetProfile } from '$types';

export class Presets {
    private ws = getSocketState();
    private presetProfiles: Record<string, PresetProfile> = {};
    private loading = false;

    private async ensurePresetProfilesLoaded(): Promise<void> {
        if (Object.keys(this.presetProfiles).length === 0 && !this.loading) {
            await this.getAllPresets();
        }
        if (this.loading) {
            await new Promise((resolve) => setTimeout(resolve, 100));
            await this.ensurePresetProfilesLoaded();
        }
    }

    async getAllPresets(): Promise<void> {
        try {
            this.loading = true;
            const response = await this.ws.sendAndWait({ type: MessageType.GET_PRESETS });
            if (response.type === 'error') {
                throw new Error(response.data);
            }
            this.presetProfiles = response.data;
            this.loading = false;
        } catch (error) {
            console.error('Error fetching presets:', error);
            throw error;
        }
    }

    async getPresetProfiles(): Promise<Record<string, PresetProfile>> {
        await this.ensurePresetProfilesLoaded();
        return this.presetProfiles;
    }

    async addPresetProfile(profile: PresetProfile): Promise<Record<string, PresetProfile>> {
        try {
            const response = await this.ws.sendAndWait({ 
                type: 'add_preset', 
                data: profile
            });
            if (response.type === 'error') {
                throw new Error(response.data);
            }
            await this.getAllPresets();
            return this.getPresetProfiles();
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
				const message = {
					type: 'update_preset',
					data: { 
						id: id,
						name: profile.name
					 }
				};
                await this.ws.send(JSON.stringify(message));
            }
            await this.getAllPresets();
            return this.getPresetProfiles();
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
                const newProfile = { ...profile, name: name};
                await this.ws.sendAndWait({ 
                    type: 'add_preset', 
                    data: newProfile 
                });
            }
            await this.getAllPresets();
            return this.getPresetProfiles();
        } catch (error) {
            console.error('Error cloning profile:', error);
            throw error;
        }
    }

    async removePresetProfiles(ids: string[]): Promise<Record<string, PresetProfile>> {
        try {
			const message = { 
				type: 'delete_preset', 
				data: ids 
			}
            await this.ws.sendAndWait(message);
            await this.getAllPresets();
            return this.getPresetProfiles();
        } catch (error) {
            console.error('Error removing preset profiles:', error);
            throw error;
        }
    }
}

export const presetsData = new Presets();