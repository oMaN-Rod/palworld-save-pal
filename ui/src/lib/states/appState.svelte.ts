// src/lib/states/appState.svelte.ts
import type { AppSettings, GamepassSave, ItemContainerSlot } from '$types';
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
	let progressMessage: string = $state('');
	let version: string = $state('');
	let settings: AppSettings = $state({ language: 'en' });
	let gamepassSaves: Record<string, GamepassSave> = $state({});

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

	// Handle selected player/pal updates
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
			players = newPlayers;
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

		get progressMessage() {
			return progressMessage;
		},
		set progressMessage(message: string) {
			progressMessage = message;
		},

		get modifiedPals() {
			return modifiedPals;
		},

		get modifiedPlayers() {
			return modifiedPlayers;
		},

		get version() {
			return version;
		},
		set version(ver: string) {
			version = ver;
		},

		get settings() {
			return settings;
		},
		set settings(newSettings: AppSettings) {
			settings = newSettings;
		},
		get gamepassSaves() {
			return gamepassSaves;
		},
		set gamepassSaves(saves: Record<string, GamepassSave>) {
			gamepassSaves = saves;
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
