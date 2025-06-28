import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

interface ExpData {
	DropEXP: number;
	NextEXP: number;
	PalNextEXP: number;
	TotalEXP: number;
	PalTotalEXP: number;
}

class ExpDataHandler {
	private loading: boolean = false;

	expData: Record<string, ExpData> = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.expData).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.expData = await sendAndWait(MessageType.GET_EXP_DATA);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching elements:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getExpData(): Promise<Record<string, ExpData>> {
		await this.ensureLoaded();
		return this.expData;
	}

	async getExpDataByLevel(level: number): Promise<ExpData> {
		await this.ensureLoaded();
		return this.expData[level.toString()];
	}

	async getLevelForExp(exp: number): Promise<number> {
		await this.ensureLoaded();
		for (let level = 1; level <= 100; level++) {
			const levelData = this.expData[level.toString()];
			if (exp < levelData.PalTotalEXP) {
				return level - 1;
			}
		}
		return 100;
	}

	async getExpForLevel(level: number): Promise<number> {
		await this.ensureLoaded();
		const levelData = this.expData[level.toString()];
		return levelData.PalTotalEXP - levelData.PalNextEXP;
	}

	async getExpProgress(level: number, exp: number): Promise<number> {
		await this.ensureLoaded();
		const levelData = this.expData[level.toString()];
		const lowerBound = levelData.PalTotalEXP - levelData.PalNextEXP;
		const upperBound = levelData.PalTotalEXP;
		return (exp - lowerBound) / (upperBound - lowerBound);
	}

	async getNextLevelExp(level: number): Promise<number> {
		await this.ensureLoaded();
		const levelData = this.expData[level.toString()];
		return levelData.PalNextEXP;
	}

	async reset(): Promise<void> {
		this.expData = {};
		await this.ensureLoaded();
	}
}

export const expData = new ExpDataHandler();
