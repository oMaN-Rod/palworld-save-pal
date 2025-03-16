import { goto } from '$app/navigation';
import { TextInputModal } from '$components';
import { send, sendAndWait } from '$lib/utils/websocketUtils';
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

const modal = getModalState();

interface ModifiedData {
	modified_pals?: Record<string, Pal>;
	modified_players?: Record<string, Player>;
	modified_guilds?: Record<string, GuildDTO>;
}

class AppState {
	players: Record<string, Player> = $state({});
	guilds: Record<string, Guild> = $state({});
	selectedPlayerUid: string = $state('');
	selectedPlayer: Player | undefined = $state(undefined);
	selectedPal: Pal | undefined = $state(undefined);
	saveFile: SaveFile | undefined = $state(undefined);
	playerSaveFiles: SaveFile[] = $state([]);
	clipboardItem: ItemContainerSlot | null = $state(null);
	progressMessage: string = $state('');
	version: string = $state('');
	settings: AppSettings = $state({ language: 'en' });
	gamepassSaves: Record<string, GamepassSave> = $state({});
	autoSave: boolean = $state(false);

	resetState() {
		this.players = {};
		this.guilds = {};
		this.selectedPlayerUid = '';
		this.selectedPlayer = undefined;
		this.selectedPal = undefined;
		this.saveFile = undefined;
		this.playerSaveFiles = [];
	}

	initData() {}

	async saveState() {
		let modifiedData: ModifiedData = {};
		let modifiedPals: [string, Pal][] = [];
		let modifiedPlayers: [string, Player][] = [];
		let modifiedGuilds: [string, GuildDTO][] = [];

		for (const player of Object.values(this.players)) {
			if (player.state === EntryState.MODIFIED) {
				const { pals, ...playerDTO } = player;
				player.state = EntryState.NONE;
				playerDTO.common_container = player.common_container;
				playerDTO.essential_container = player.essential_container;
				playerDTO.weapon_load_out_container = player.weapon_load_out_container;
				playerDTO.player_equipment_armor_container = player.player_equipment_armor_container;
				playerDTO.food_equip_container = player.food_equip_container;
				modifiedPlayers = [...modifiedPlayers, [player.uid, playerDTO]];
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

		for (const guild of Object.values(this.guilds)) {
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
			this.autoSave = true;
		}
		await sendAndWait(MessageType.UPDATE_SAVE_FILE, modifiedData);
		await new Promise((resolve) => setTimeout(resolve, 500));
		this.autoSave = false;
	}

	async writeSave() {
		if (!this.saveFile) return;
		await this.saveState();
		if (this.saveFile.type === 'gamepass') {
			const split = this.saveFile.world_name?.split('PSP-') || [];
			const baseName = split.length > 1 ? split[0].trim() : this.saveFile.world_name || 'PSP';
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

			send(MessageType.SAVE_MODDED_SAVE, result);
		} else if (this.saveFile.type === 'steam') {
			await goto('/loading');

			send(MessageType.SAVE_MODDED_SAVE, null);
		}
	}
}

const appState = new AppState();
export const getAppState = () => appState;
