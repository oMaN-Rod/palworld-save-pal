import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';

export interface FriendshipRank {
	rank: number;
	required_point: number;
}

export type FriendshipData = Record<string, FriendshipRank>;

class FriendshipDataHandler {
	private loading = false;
	friendshipData: FriendshipData = $state({});

	private async ensureLoaded(): Promise<void> {
		if (Object.keys(this.friendshipData).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.friendshipData = await sendAndWait(MessageType.GET_FRIENDSHIP_DATA);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching friendship data:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureLoaded();
		}
	}

	async getFriendshipData(): Promise<FriendshipData> {
		await this.ensureLoaded();
		return this.friendshipData;
	}

	async reset(): Promise<void> {
		this.friendshipData = {};
		await this.ensureLoaded();
	}
}

export const friendshipData = new FriendshipDataHandler();
