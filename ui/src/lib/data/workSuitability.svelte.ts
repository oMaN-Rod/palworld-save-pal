import { getSocketState } from '$states';
import { MessageType, type WorkSuitability } from '$types';

interface WorkSuitabilityData {
	localized_name?: string;
	description?: string;
}

class WorkSuitabilities {
	private loading: boolean = false;
	private ws = getSocketState();

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
		if (this.loading) {
			while (this.loading) {
				await new Promise((resolve) => setTimeout(resolve, 100));
			}
			return;
		}

		this.loading = true;
		try {
			const response = await this.ws.sendAndWait({ type: MessageType.GET_WORK_SUITABILITY });
			if (response.type === MessageType.GET_WORK_SUITABILITY) {
				this.workSuitability = response.data;
			} else {
				throw new Error('Failed to fetch work suitability data');
			}
		} finally {
			this.loading = false;
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
