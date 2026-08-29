import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

interface ExpData {
	DropEXP: number;
	NextEXP: number;
	PalNextEXP: number;
	TotalEXP: number;
	PalTotalEXP: number;
	// Per-action EXP yields from DT_PalExpTable (Palworld 1.x). Reference data
	// only -- these are not persisted per-character, so leveling never writes them.
	BuildEXP: number;
	CraftEXP: number;
	PalBuildEXP: number;
	PalCraftEXP: number;
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

	get maxLevel(): number {
		let max = 1;
		for (const key of Object.keys(this.expData)) {
			const level = Number(key);
			if (Number.isFinite(level) && level > max) max = level;
		}
		return max;
	}

	private rowFor(level: number): ExpData {
		const clamped = Math.min(Math.max(Math.trunc(level), 1), this.maxLevel);
		return this.expData[clamped.toString()];
	}

	async getExpData(): Promise<Record<string, ExpData>> {
		await this.ensureLoaded();
		return this.expData;
	}

	async getExpDataByLevel(level: number): Promise<ExpData> {
		await this.ensureLoaded();
		return this.rowFor(level);
	}

	async getLevelForExp(exp: number): Promise<number> {
		await this.ensureLoaded();
		const max = this.maxLevel;
		for (let level = 1; level <= max; level++) {
			const levelData = this.expData[level.toString()];
			if (exp < levelData.PalTotalEXP) {
				return level - 1;
			}
		}
		return max;
	}

	async getExpForLevel(level: number): Promise<number> {
		await this.ensureLoaded();
		const levelData = this.rowFor(level);
		return levelData.PalTotalEXP - levelData.PalNextEXP;
	}

	async getExpProgress(level: number, exp: number): Promise<number> {
		await this.ensureLoaded();
		const levelData = this.rowFor(level);
		const lowerBound = levelData.PalTotalEXP - levelData.PalNextEXP;
		const upperBound = levelData.PalTotalEXP;
		return (exp - lowerBound) / (upperBound - lowerBound);
	}

	async getNextLevelExp(level: number): Promise<number> {
		await this.ensureLoaded();
		return this.rowFor(level).PalNextEXP;
	}

	async reset(): Promise<void> {
		this.expData = {};
		await this.ensureLoaded();
	}
}

export const expData = new ExpDataHandler();
