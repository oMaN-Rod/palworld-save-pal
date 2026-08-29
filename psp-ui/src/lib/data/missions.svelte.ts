import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Mission, type MissionType } from '$types';
import { normalizeKeys } from '$utils';

class Missions {
	private loading = false;
	private keyMap: Record<string, string> = $state({});

	missions: Record<string, Mission> = $state({});

	private async ensureMissionsLoaded(): Promise<void> {
		if (Object.keys(this.missions).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.missions = await sendAndWait(MessageType.GET_MISSIONS);
				this.keyMap = normalizeKeys(Object.keys(this.missions));
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching missions:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureMissionsLoaded();
		}
	}

	getByKey(key: string): Mission | undefined {
		try {
			return this.missions[this.keyMap[key.toLowerCase()]];
		} catch (error) {
			console.error('Error getting mission data:', error);
		}
	}

	getByType(type: MissionType): Mission[] {
		return Object.values(this.missions).filter((mission) => mission.quest_type === type);
	}

	getMainMissions(): Mission[] {
		return this.getByType('Main');
	}

	getSubMissions(): Mission[] {
		return this.getByType('Sub');
	}

	async reset(): Promise<void> {
		this.missions = {};
		await this.ensureMissionsLoaded();
	}
}

export const missionsData = new Missions();