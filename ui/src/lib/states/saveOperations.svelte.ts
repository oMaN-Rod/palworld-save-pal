import { goto } from '$app/navigation';
import { TextInputModal } from '$components';
import { send, sendAndWait } from '$lib/utils/websocketUtils';
import { upsState } from '$states';
import type { GuildDTO, ItemContainer, UPSPal } from '$types';
import { EntryState, MessageType, type Pal, type Player } from '$types';
import { deepCopy } from '$utils';
import { getModalState } from './modalState.svelte';
import { getPalEditorState } from './palEditorState.svelte';
import type { AppState } from './appState.svelte';

interface ModifiedData {
	modified_pals?: Record<string, Pal>;
	modified_dps_pals?: Record<number, Pal>;
	modified_players?: Record<string, Player>;
	modified_guilds?: Record<string, GuildDTO>;
	modified_gps_pals?: Record<number, Pal>;
}

export function processPlayers(state: AppState) {
	let modifiedPlayers: [string, Player][] = [];
	let modifiedPals: [string, Pal][] = [];
	let modifiedDpsPals: [string, Pal][] = [];

	for (const player of Object.values(state.players)) {
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
					if (!pal.__ups_source) {
						modifiedDpsPals = [...modifiedDpsPals, [index, pal]];
					}
					pal.state = EntryState.NONE;
				}
			}
		}
	}
	return { modifiedPlayers, modifiedPals, modifiedDpsPals };
}

export function processGuilds(state: AppState) {
	let modifiedGuilds: [string, GuildDTO][] = [];
	let modifiedPals: [string, Pal][] = [];
	for (const guild of Object.values(state.guilds ?? {})) {
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
					guildClone.bases[base.id].storage_containers =
						Object.fromEntries(modifiedContainers);
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
	return { modifiedGuilds, modifiedPals };
}

export async function addNewUpspal(state: AppState, pal: Pal) {
	const palDto = {
		instance_id: '00000000-0000-0000-0000-000000000000',
		owner_uid: null,
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
		storage_id: '00000000-0000-0000-0000-000000000000',
		storage_slot: 0,
		group_id: null,
		learned_skills: pal.learned_skills,
		active_skills: pal.active_skills,
		passive_skills: pal.passive_skills,
		work_suitability: pal.work_suitability,
		is_sick: pal.is_sick,
		friendship_point: pal.friendship_point
	};

	const upsPal: UPSPal = await sendAndWait(MessageType.ADD_UPS_PAL, {
		pal_dto: palDto,
		source_save_file: undefined,
		source_player_uid: undefined,
		source_player_name: undefined,
		source_storage_type: 'manual_add',
		source_storage_slot: undefined,
		collection_id: undefined,
		tags: [],
		notes: 'Created via Add Pal feature'
	});

	const palWithMetadata = {
		...(upsPal.pal_data as Pal),
		__ups_source: true,
		__ups_id: upsPal.id,
		__ups_new: false
	};

	upsState.pals = [...upsState.pals, upsPal];
	upsState.pagination.totalCount++;
	getPalEditorState().open(palWithMetadata);
}

export async function saveUpspalChanges(pal: Pal) {
	if (!pal.__ups_id) {
		throw new Error('UPS pal ID not found');
	}

	const updates = {
		nickname: pal.nickname,
		level: pal.level,
		pal_data: {
			...pal
		}
	};
	upsState.updatePal(pal.__ups_id, updates);
}

export async function saveState(state: AppState) {
	console.log('Saving state...');
	let modifiedData: ModifiedData = {};
	let modifiedPals: [string, Pal][] = [];
	let modifiedDspPals: [string, Pal][] = [];
	let modifiedGspPals: [string, Pal][] = [];
	let modifiedPlayers: [string, Player][] = [];
	let modifiedGuilds: [string, GuildDTO][] = [];

	// Handle UPS pal modifications and new UPS pals
	if (
		state.selectedPal &&
		state.selectedPal.state === EntryState.MODIFIED &&
		state.selectedPal.__ups_source
	) {
		try {
			if (state.selectedPal.__ups_new) {
				await addNewUpspal(state, state.selectedPal);
			} else {
				await saveUpspalChanges(state.selectedPal);
			}
			state.selectedPal.state = EntryState.NONE;
		} catch (error) {
			console.error('Failed to save UPS pal changes:', error);
		}
	}

	if (state.players && Object.entries(state.players).length > 0) {
		const {
			modifiedPlayers: modsPlayers,
			modifiedPals: modsPals,
			modifiedDpsPals: modsDps
		} = processPlayers(state);
		modifiedPlayers = modsPlayers;
		modifiedPals = modsPals;
		modifiedDspPals = modsDps;
	} else {
		console.log('No players to process for modifications');
	}

	if (state.guilds && Object.entries(state.guilds).length > 0) {
		const { modifiedGuilds: modsGuilds, modifiedPals: modsPals } = processGuilds(state);
		modifiedGuilds = modsGuilds;
		modifiedPals = [...modifiedPals, ...modsPals];
	} else {
		console.log('No guilds to process for modifications');
	}

	if (state.gps && Object.values(state.gps).some((p) => p.state === EntryState.MODIFIED)) {
		for (const [id, pal] of Object.entries(state.gps)) {
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
		state.autoSave = true;
	}
	await sendAndWait(MessageType.UPDATE_SAVE_FILE, modifiedData);
	await new Promise((resolve) => setTimeout(resolve, 500));
	state.autoSave = false;
}

export async function writeSave(state: AppState) {
	const modal = getModalState();
	if (!state.saveFile) return;
	await saveState(state);
	if (state.saveFile.type === 'gamepass') {
		const split = state.saveFile.world_name?.split('PSP-') || [];
		const baseName = split.length > 1 ? split[0].trim() : state.saveFile.world_name || 'PSP';
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
	} else if (state.saveFile.type === 'steam') {
		await goto('/loading');

		send(MessageType.SAVE_MODDED_SAVE, null);
	}
}