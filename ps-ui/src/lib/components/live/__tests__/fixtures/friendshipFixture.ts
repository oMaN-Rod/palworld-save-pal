import { friendshipData } from '$lib/data/friendship.svelte';
import type { FriendshipData } from '$lib/data/friendship.svelte';

export function seedFriendship(): void {
	const ranks: FriendshipData = {
		'1': { rank: 1, required_point: 0 },
		'2': { rank: 2, required_point: 100 }
	};
	friendshipData.friendshipData = ranks;
}

export function clearFriendship(): void {
	friendshipData.friendshipData = {};
}
