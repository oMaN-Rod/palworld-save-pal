import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type WorkSuitability } from '$types';

interface WorkSuitabilityData {
	localized_name?: string;
	description?: string;
}

const WORK_SUITABILITY_SEED: Record<WorkSuitability, WorkSuitabilityData> = {
	EmitFlame: {},
	Watering: {},
	Seeding: {},
	GenerateElectricity: {},
	Handcraft: {},
	Collection: {},
	Deforest: {},
	Mining: {},
	OilExtraction: {},
	ProductMedicine: {},
	Cool: {},
	Transport: {},
	MonsterFarm: {}
};

/** Build-time source for the 13 work suitability keys; the store below is empty until runtime. */
export const WORK_SUITABILITY_KEYS = Object.keys(WORK_SUITABILITY_SEED) as WorkSuitability[];

class WorkSuitabilities {
	private loading: boolean = false;

	workSuitability: Record<WorkSuitability, WorkSuitabilityData> = $state({
		...WORK_SUITABILITY_SEED
	});

	private async ensureLoaded(): Promise<void> {
		if (!this.loading) {
			try {
				this.loading = true;
				this.workSuitability = await sendAndWait(MessageType.GET_WORK_SUITABILITY);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching work suitability:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async reset(): Promise<void> {
		this.workSuitability = { ...WORK_SUITABILITY_SEED };
		await this.ensureLoaded();
	}
}

export const workSuitabilityData = new WorkSuitabilities();
