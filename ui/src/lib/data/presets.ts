import { getSocketState } from '$states/websocketState.svelte';
import { MessageType, type PresetProfile } from '$types';

export class Presets {
    private ws = getSocketState();

    async getPresetProfiles(): Promise<Record<string, PresetProfile>> {
        try {
            const response = await this.ws.sendAndWait({ type: MessageType.GET_PRESETS });
            if (response.type === 'error') {
                throw new Error(response.data);
            }
            return response.data;
        } catch (error) {
            console.error('Error fetching presets:', error);
            throw error;
        }
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
            return this.getPresetProfiles();
        } catch (error) {
            console.error('Error removing preset profiles:', error);
            throw error;
        }
    }
}

export const presetsData = new Presets();