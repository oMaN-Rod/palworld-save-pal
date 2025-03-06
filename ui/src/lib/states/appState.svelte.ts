import { goto } from '$app/navigation';
import { TextInputModal } from '$components';
import type {
	AppSettings,
	GamepassSave,
	Guild,
	GuildDTO,
	ItemContainer,
	ItemContainerSlot
} from '$types';
import { EntryState, MessageType, type Pal, type Player, type SaveFile } from '$types';
import { getModalState } from './modalState.svelte';
import { getToastState } from './toastState.svelte';
import { getSocketState } from './websocketState.svelte';

const ws = getSocketState();
const toast = getToastState();
const modal = getModalState();

interface ModifiedData {
	modified_pals?: Record<string, Pal>;
	modified_players?: Record<string, Player>;
	modified_guilds?: Record<string, GuildDTO>;
}

export function createAppState() {
	let players: Record<string, Player> = $state({});
	let guilds: Record<string, Guild> = $state({});
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
	let autoSave: boolean = $state(false);

	function resetState() {
		players = {};
		guilds = {};
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

	async function saveState() {
		let modifiedData: ModifiedData = {};
		let modifiedPals: [string, Pal][] = [];
		let modifiedPlayers: [string, Player][] = [];
		let modifiedGuilds: [string, GuildDTO][] = [];

		for (const player of Object.values(appState.modifiedPlayers)) {
			if (player.state === EntryState.MODIFIED) {
				const { pals, ...playerWithoutPals } = player;
				player.state = EntryState.NONE;
				modifiedPlayers = [...modifiedPlayers, [player.uid, playerWithoutPals]];
			}
			if (player.pals) {
				for (const pal of Object.values(player.pals)) {
					if (pal.state === EntryState.MODIFIED) {
						modifiedPals = [...modifiedPals, [pal.instance_id, pal]];
						pal.state = EntryState.NONE;
					}
				}
			}
		}

		for (const guild of Object.values(appState.guilds)) {
			if (guild.bases) {
				for (const base of Object.values(guild.bases)) {
					if (base.pals) {
						for (const pal of Object.values(base.pals)) {
							if (pal.state === EntryState.MODIFIED) {
								modifiedPals = [...modifiedPals, [pal.instance_id, pal]];
								pal.state = EntryState.NONE;
							}
						}
					}
					let modifiedContainers: [string, ItemContainer][] = [];
					for (const container of Object.values(base.storage_containers)) {
						if (container.state === EntryState.MODIFIED) {
							modifiedContainers = [...modifiedContainers, [container.id, container]];
							container.state = EntryState.NONE;
						}
					}
					if (modifiedContainers.length > 0) {
						modifiedGuilds = [
							...modifiedGuilds,
							[
								guild.id,
								{
									base: { id: base.id, storage_containers: Object.fromEntries(modifiedContainers) }
								}
							]
						];
					}
				}
			}
			if (guild.guild_chest && guild.guild_chest.state === EntryState.MODIFIED) {
				modifiedGuilds = [...modifiedGuilds, [guild.id, { guild_chest: guild.guild_chest }]];
				guild.guild_chest.state = EntryState.NONE;
			}
			if (guild.state === EntryState.MODIFIED) {
				modifiedGuilds = [...modifiedGuilds, [guild.id, guild]];
				guild.state = EntryState.NONE;
			}
		}

		if (modifiedPals.length === 0 && modifiedPlayers.length === 0 && modifiedGuilds.length === 0) {
			console.log('No modifications to save');
			return;
		}

		if (modifiedPals.length > 0) {
			modifiedData.modified_pals = Object.fromEntries(modifiedPals);
		}

		if (modifiedPlayers.length > 0) {
			modifiedData.modified_players = Object.fromEntries(modifiedPlayers);
		}

		if (modifiedGuilds.length > 0) {
			modifiedData.modified_guilds = Object.fromEntries(modifiedGuilds);
		}

		if (modifiedPals.length > 0 || modifiedPlayers.length > 0 || modifiedGuilds.length > 0) {
			autoSave = true;
		}

		const data = { type: MessageType.UPDATE_SAVE_FILE, data: modifiedData };

		await ws.sendAndWait(data);
		await new Promise((resolve) => setTimeout(resolve, 500));
		autoSave = false;
	}

	async function writeSave() {
		if (!saveFile) return;
		await saveState();
		if (saveFile.type === 'gamepass') {
			const split = saveFile.world_name?.split('PSP-') || [];
			const baseName = split.length > 1 ? split[0].trim() : saveFile.world_name || 'PSP';
			const timestamp = new Date()
				.toLocaleString('en-GB', {
					year: '2-digit',
					month: '2-digit',
					day: '2-digit',
					hour: '2-digit',
					minute: '2-digit'
				})
				.replace(/[/,]/g, '')
				.replace(/\s/g, '_');
			// @ts-ignore
			const result = await modal.showModal<string>(TextInputModal, {
				title: 'Edit World Name',
				value: `${baseName} PSP-${timestamp}`
			});

			if (!result) return;

			await goto('/loading');

			ws.send(JSON.stringify({ type: MessageType.SAVE_MODDED_SAVE, data: result }));
		} else if (saveFile.type === 'steam') {
			await goto('/loading');

			ws.send(JSON.stringify({ type: MessageType.SAVE_MODDED_SAVE, data: null }));
		}
	}

	return {
		get autoSave() {
			return autoSave;
		},
		set autoSave(value: boolean) {
			autoSave = value;
		},
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
		get guilds() {
			return guilds;
		},
		set guilds(newGuilds: Record<string, Guild>) {
			guilds = newGuilds;
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
		saveState,
		writeSave,
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
