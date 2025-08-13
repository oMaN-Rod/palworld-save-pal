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
import { deepCopy } from '$utils';
import { getModalState } from './modalState.svelte';

const modal = getModalState();

interface ModifiedData {
	modified_pals?: Record<string, Pal>;
	modified_dps_pals?: Record<number, Pal>;
	modified_players?: Record<string, Player>;
	modified_guilds?: Record<string, GuildDTO>;
	modified_gps_pals?: Record<number, Pal>;
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
	gps: Record<number, Pal> = $state({});

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

	async saveUpspalChanges(pal: Pal) {
		if (!pal.__ups_id) {
			throw new Error('UPS pal ID not found');
		}

		// Create the updates object from pal data
		const updates = {
			nickname: pal.nickname,
			level: pal.level,
			pal_data: {
				instance_id: pal.instance_id,
				character_id: pal.character_id,
				nickname: pal.nickname,
				level: pal.level,
				exp: pal.exp,
				rank: pal.rank,
				rank_hp: pal.rank_hp,
				rank_attack: pal.rank_attack,
				rank_defense: pal.rank_defense,
				rank_craftspeed: pal.rank_craftspeed,
				talent_hp: pal.talent_hp,
				talent_shot: pal.talent_shot,
				talent_defense: pal.talent_defense,
				hp: pal.hp,
				max_hp: pal.max_hp,
				sanity: pal.sanity,
				stomach: pal.stomach,
				is_lucky: pal.is_lucky,
				is_boss: pal.is_boss,
				gender: pal.gender,
				is_tower: pal.is_tower,
				learned_skills: pal.learned_skills,
				active_skills: pal.active_skills,
				passive_skills: pal.passive_skills,
				work_suitability: pal.work_suitability,
				is_sick: pal.is_sick,
				friendship_point: pal.friendship_point,
			}
		};

		// Send UPDATE_UPS_PAL message
		await sendAndWait(MessageType.UPDATE_UPS_PAL, {
			pal_id: pal.__ups_id,
			updates: updates
		});
	}

	async saveState() {
		let modifiedData: ModifiedData = {};
		let modifiedPals: [string, Pal][] = [];
		let modifiedDspPals: [string, Pal][] = [];
		let modifiedGspPals: [string, Pal][] = [];
		let modifiedPlayers: [string, Player][] = [];
		let modifiedGuilds: [string, GuildDTO][] = [];

		// Handle UPS pal modifications
		if (this.selectedPal && this.selectedPal.state === EntryState.MODIFIED && this.selectedPal.__ups_source) {
			try {
				// Save UPS pal changes back to UPS
				await this.saveUpspalChanges(this.selectedPal);
				this.selectedPal.state = EntryState.NONE;
			} catch (error) {
				console.error('Failed to save UPS pal changes:', error);
			}
		}

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
						// Skip UPS pals - they're handled separately
						if (!pal.__ups_source) {
							modifiedPals = [...modifiedPals, [pal.instance_id, pal]];
						}
						pal.state = EntryState.NONE;
					}
				}
			}
			if (player.dps) {
				for (const [index, pal] of Object.entries(player.dps)) {
					if (pal && pal.state === EntryState.MODIFIED) {
						pal.owner_uid = player.uid;
						// Skip UPS pals - they're handled separately
						if (!pal.__ups_source) {
							modifiedDspPals = [...modifiedDspPals, [index, pal]];
						}
						pal.state = EntryState.NONE;
					}
				}
			}
		}

		for (const guild of Object.values(this.guilds)) {
			const guildClone = deepCopy(guild);
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
						guildClone.bases[base.id].storage_containers = Object.fromEntries(modifiedContainers);
						guild.state = EntryState.MODIFIED;
					}
				}
			}
			if (guild.guild_chest && guild.guild_chest.state === EntryState.MODIFIED) {
				guild.guild_chest.state = EntryState.NONE;
			} else if (guildClone.guild_chest) {
				guildClone.guild_chest = undefined;
			}
			if (guild.state === EntryState.MODIFIED) {
				modifiedGuilds = [...modifiedGuilds, [guildClone.id, guildClone]];
				guild.state = EntryState.NONE;
			}
		}

		if (this.gps && Object.values(this.gps).some((p) => p.state === EntryState.MODIFIED)) {
			for (const [id, pal] of Object.entries(this.gps)) {
				if (pal && pal.state === EntryState.MODIFIED) {
					modifiedGspPals = [...modifiedGspPals, [id, pal]];
					pal.state = EntryState.NONE;
				}
			}
		}

		if (
			modifiedPals.length === 0 &&
			modifiedPlayers.length === 0 &&
			modifiedGuilds.length === 0 &&
			modifiedDspPals.length === 0 &&
			modifiedGspPals.length === 0
		) {
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

		if (modifiedDspPals.length > 0) {
			modifiedData.modified_dps_pals = Object.fromEntries(modifiedDspPals);
		}

		if (modifiedGspPals.length > 0) {
			modifiedData.modified_gps_pals = Object.fromEntries(modifiedGspPals);
		}

		if (
			modifiedPals.length > 0 ||
			modifiedPlayers.length > 0 ||
			modifiedGuilds.length > 0 ||
			modifiedDspPals.length > 0
		) {
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
