import { getSocketState } from '$states/websocketState.svelte';
import { type PalData, MessageType } from '$types';


export class Pals {
    private ws = getSocketState();
    private loading = false;
    
    pals: Record<string, PalData> = $state({});

    private async ensurePalsLoaded(): Promise<void> {
        if (Object.keys(this.pals).length === 0 && !this.loading) {
            try {
                this.loading = true;
                const response = await this.ws.sendAndWait({ 
                    type: MessageType.GET_PALS 
                });
                if (response.type === 'error') {
                    throw new Error(response.data);
                }
                this.pals = response.data;
                this.loading = false;
            } catch (error) {
                console.error('Error fetching pals:', error);
                throw error;
            }
        }
        if (this.loading) {
            await new Promise((resolve) => setTimeout(resolve, 100));
            await this.ensurePalsLoaded();
        }
    }

    async getPalInfo(key: string): Promise<PalData | undefined> {
        await this.ensurePalsLoaded();
        return this.pals[key];
    }

    async searchByLocalizedName(localizedName: string): Promise<PalData | undefined> {
        await this.ensurePalsLoaded();
        return Object.values(this.pals).find((pal) => pal.localized_name.toLowerCase() === localizedName.toLowerCase());
    }

    async getAllPals(): Promise<[string, PalData][]> {
        await this.ensurePalsLoaded();
        return Object.entries(this.pals);
    }
}

export const palsData = new Pals();
