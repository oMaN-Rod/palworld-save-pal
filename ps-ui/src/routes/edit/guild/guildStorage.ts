/** Container keys that are storage in the save but not storage to a player. */
export const IGNORED_CONTAINER_KEYS = ['None', 'Empty', 'TreasureBox', 'PalEgg', 'CommonDropItem'];

/** Summed across every container in the base. */
export type GuildInventoryItem = {
	static_id: string;
	containers: Record<string, number>;
	total_count: number;
};
