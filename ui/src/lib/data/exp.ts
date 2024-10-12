// ui/src/lib/data/exp.ts

import { getSocketState } from '$states';
import { MessageType } from '$types';

interface ExpData {
	DropEXP: number;
	NextEXP: number;
	PalNextEXP: number;
	TotalEXP: number;
	PalTotalEXP: number;
}

class ExpDataHandler {
	private expData: Record<string, ExpData> = {};
	private loading: boolean = false;
	private ws = getSocketState();

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.expData).length > 0) return;
		if (this.loading) {
			while (this.loading) {
				await new Promise((resolve) => setTimeout(resolve, 100));
			}
			return;
		}

		this.loading = true;
		try {
			const response = await this.ws.sendAndWait({ type: MessageType.GET_EXP_DATA });
			if (response.type === MessageType.GET_EXP_DATA) {
				this.expData = response.data;
			} else {
				throw new Error('Failed to fetch exp data');
			}
		} finally {
			this.loading = false;
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
}

export const expData = new ExpDataHandler();
