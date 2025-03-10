import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type WorkSuitability } from '$types';

interface WorkSuitabilityData {
	localized_name?: string;
	description?: string;
}

class WorkSuitabilities {
	private loading: boolean = false;

	workSuitability: Record<WorkSuitability, WorkSuitabilityData> = $state({
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
	});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.workSuitability).length === 0 && !this.loading) {
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
		this.workSuitability = {
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
		await this.ensureLoaded();
	}
}

export const workSuitabilityData = new WorkSuitabilities();
