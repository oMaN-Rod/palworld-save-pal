import { palsData } from '$lib/data/pals';
import type { ItemContainerSlot } from '$types';
import { type Pal, type Player, type SaveFile } from '$types';
import { getSocketState } from './websocketState.svelte';

const ws = getSocketState();

export function createAppState() {
	let players: Record<string, Player> = $state({});
	let selectedPlayerUid: string = $state('');
	let selectedPlayer: Player | undefined = $state(undefined);
	let selectedPal: Pal | undefined = $state(undefined);
	let saveFile: SaveFile | undefined = $state(undefined);
	let playerSaveFiles: SaveFile[] = $state([]);
	let modifiedPals: Record<string, Pal> = $state({});
	let modifiedPlayers: Record<string, Player> = $state({});
	let clipboardItem: ItemContainerSlot | null = $state(null);

	function resetState() {
		players = {};
		selectedPlayerUid = '';
		selectedPlayer = undefined;
		selectedPal = undefined;
		saveFile = undefined;
		playerSaveFiles = [];
		modifiedPals = {};
		modifiedPlayers = {};
	}

	function setPlayers(newPlayers: Record<string, Player>) {
		Object.entries(newPlayers).forEach(([key, player]) => {
			try {
				if (player.pals) {
					Object.values(player.pals).forEach(async (pal) => {
						const palInfo = await palsData.getPalInfo(pal.character_id);
						if (!palInfo) {
							console.error(`Failed to find pal info for`, pal);
						}
						pal.name = palInfo?.localized_name || pal.character_id;
					});
				}

				players[key] = player;
			} catch (error) {
				console.error(`Failed to parse player data for key ${key}:`, error);
			}
		});
	}

	function setSelectedPal(pal: Pal | undefined) {
		selectedPal = pal;
		if (pal) {
			modifiedPals[pal.instance_id] = pal;
		}
	}

	function setSelectedPlayer(player: Player | undefined) {
		selectedPlayer = player;
		selectedPal = undefined;
		if (player) {
			modifiedPlayers[player.uid] = player;
		}
	}

	return {
		get clipboardItem() {
			return clipboardItem;
		},
		set clipboardItem(item: ItemContainerSlot | null) {
			clipboardItem = item;
		},
		get players() {
			return players;
		},
		set players(newPlayers: Record<string, Player>) {
			setPlayers(newPlayers);
		},

		get selectedPlayerUid() {
			return selectedPlayerUid;
		},
		set selectedPlayerUid(uid: string) {
			selectedPlayerUid = uid;
		},

		get selectedPlayer() {
			return selectedPlayer as Player;
		},
		set selectedPlayer(player: Player | undefined) {
			setSelectedPlayer(player);
		},

		get selectedPal() {
			return selectedPal;
		},
		set selectedPal(pal: Pal | undefined) {
			setSelectedPal(pal);
		},

		get saveFile() {
			return saveFile;
		},
		set saveFile(file: SaveFile | undefined) {
			saveFile = file;
		},

		get playerSaveFiles() {
			return playerSaveFiles;
		},
		set playerSaveFiles(files: SaveFile[]) {
			playerSaveFiles = files;
		},

		get modifiedPals() {
			return modifiedPals;
		},

		get modifiedPlayers() {
			return modifiedPlayers;
		},
		resetState,
		resetModified() {
			modifiedPlayers = {};
			modifiedPals = {};
		}
	};
}

let appState: ReturnType<typeof createAppState>;

export function getAppState() {
	if (!appState) {
		appState = createAppState();
	}
	return appState;
}
