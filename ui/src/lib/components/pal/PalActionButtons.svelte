<script lang="ts">
	import { PalPresetSelectModal, PresetConfigModal, TextInputModal } from '$components/modals';
	import { CornerDotButton, Tooltip } from '$components/ui';
	import {
		defaultPresetConfig,
		type ElementType,
		EntryState,
		type Pal,
		PalGender,
		type PalPresetConfig,
		type PresetProfile
	} from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { palsData, elementsData, presetsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getModalState, getNavigationState, getToastState } from '$states';
	import { BicepsFlexed, Bug, Edit, Play, Save } from 'lucide-svelte';
	import { assetLoader, handleMaxOutPal, editLucky, editAlpha } from '$utils';
	import { staticIcons } from '$types/icons';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

	let {
		pal = $bindable(),
		showActions = true,
		popup = false
	}: {
		pal: Pal;
		showActions?: boolean;
		popup?: boolean;
	} = $props();

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();
	const nav = getNavigationState();

	function getPalElementTypes(character_id: string): ElementType[] | undefined {
		const palData = palsData.getByKey(character_id);
		if (!palData) return undefined;
		return palData.element_types.length > 0 ? palData.element_types : undefined;
	}

	function getPalElementBadge(elementType: string): string | undefined {
		const elementObj = elementsData.getByKey(elementType);
		if (!elementObj) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.webp`);
	}

	async function handleEditNickname() {
		if (!pal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: m.edit_entity({ entity: m.nickname() }),
			value: pal.nickname || pal.name
		});
		if (!result) return;
		pal.nickname = result;
		pal.state = EntryState.MODIFIED;
		if (appState.selectedPlayer && appState.selectedPlayer.pals)
			appState.selectedPlayer.pals[pal.instance_id].nickname = result;
	}

	function handleEditGender() {
		if (!pal) return;

		let genderCycle: Record<PalGender, PalGender> = {
			[PalGender.MALE]: PalGender.FEMALE,
			[PalGender.FEMALE]: PalGender.MALE,
			[PalGender.NONE]: PalGender.MALE
		};

		const palData = palsData.getByKey(pal.character_key);
		if (palData && !palData.is_pal) {
			genderCycle[PalGender.FEMALE] = PalGender.NONE;
		}

		pal.gender = genderCycle[pal.gender] ?? PalGender.MALE;
		pal.state = EntryState.MODIFIED;
	}

	function formatBossCharacterId() {
		pal.character_id = pal.character_id.replace('Boss_', 'BOSS_');
		if (pal && (pal.is_boss || pal.is_lucky) && !pal.character_id.startsWith('BOSS_')) {
			pal.character_id = `BOSS_${pal.character_id}`;
		} else if (pal && !pal.is_boss && !pal.is_lucky && pal.character_id.startsWith('BOSS_')) {
			pal.character_id = pal.character_id.replace('BOSS_', '');
		}
	}

	function handleEditLucky() {
		const [type, valid] = editLucky(pal);
		if (!valid) {
			toast.add(m.pal_cannot_be_trait({ type, trait: m.lucky() }), undefined, 'warning');
		}
	}

	function handleEditAlpha() {
		const [type, valid] = editAlpha(pal);
		if (!valid) {
			toast.add(m.pal_cannot_be_trait({ type, trait: m.alpha() }), undefined, 'warning');
		}
	}

	async function handleSelectPreset() {
		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: m.select_entity({ entity: c.preset }),
			selectedPals: [{ character_id: pal.character_id, character_key: pal.character_key }]
		});
		if (!result) return;

		const presetProfile = presetsData.presetProfiles[result];

		for (const [key, value] of Object.entries(presetProfile.pal_preset!)) {
			if (key === 'character_id') continue;
			if (key === 'lock' && value) {
				pal.character_id = presetProfile.pal_preset?.character_id as string;
			}
			if (key === 'is_boss' && value && pal.is_lucky) {
				pal.is_boss = true;
				pal.is_lucky = false;
			}
			if (key === 'is_lucky' && value && pal.is_boss) {
				pal.is_boss = false;
				pal.is_lucky = true;
			} else if (value !== null) {
				(pal as Record<string, any>)[key] = value;
			}
		}
		pal.state = EntryState.MODIFIED;
	}

	async function handleSavePreset() {
		const element = palsData.getByKey(pal.character_key)?.element_types[0];
		// @ts-ignore
		const result = await modal.showModal(PresetConfigModal, {
			config: defaultPresetConfig,
			palName: pal.name,
			element
		});
		if (!result) return;

		const { name, config } = result as { name: string; config: PalPresetConfig };

		const newPreset = {
			name: name,
			type: 'pal_preset',
			pal_preset: {
				lock: config.lock,
				character_id: pal.character_id,
				is_lucky: config.is_lucky ? pal.is_lucky : null,
				is_boss: config.is_boss ? pal.is_boss : null,
				gender: config.gender ? pal.gender : null,
				rank_hp: config.rank_hp ? pal.rank_hp : null,
				rank_attack: config.rank_attack ? pal.rank_attack : null,
				rank_defense: config.rank_defense ? pal.rank_defense : null,
				rank_craftspeed: config.rank_craftspeed ? pal.rank_craftspeed : null,
				talent_hp: config.talent_hp ? pal.talent_hp : null,
				talent_shot: config.talent_shot ? pal.talent_shot : null,
				talent_defense: config.talent_defense ? pal.talent_defense : null,
				rank: config.rank ? pal.rank : null,
				level: config.level ? pal.level : null,
				learned_skills: config.learned_skills ? pal.learned_skills : null,
				active_skills: config.active_skills ? pal.active_skills : null,
				passive_skills: config.passive_skills ? pal.passive_skills : null,
				work_suitability: config.work_suitability ? pal.work_suitability : null,
				sanity: config.sanity ? pal.sanity : null,
				exp: config.exp ? pal.exp : null,
				element: element,
				lock_element: config.lock_element,
				nickname: config.nickname ? pal.nickname : null,
				filtered_nickname: config.filtered_nickname ? pal.nickname : null,
				stomach: config.stomach ? pal.stomach : null,
				hp: config.hp ? pal.hp : null,
				friendship_point: config.friendship_point ? pal.friendship_point : null
			}
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
	}
</script>

<div class={cn('flex flex-wrap items-start gap-2', popup ? '2xl:flex-col' : '')}>
	<h6 class="h6 min-w-0 grow truncate">
		{pal.nickname || pal.name}
	</h6>
	<div class="flex space-x-2">
		{#if appState.settings.debug_mode && showActions}
			<Tooltip position="bottom" label={m.debug()}>
				<CornerDotButton
					onClick={() => {
						nav.saveAndNavigate(
							`/debug?guildId=${appState.selectedPlayer?.guild_id}&playerId=${appState.selectedPlayer!.uid}&palId=${appState.selectedPal!.instance_id}`
						);
					}}
					class="h-8 w-8 p-1"
				>
					<Bug />
				</CornerDotButton>
			</Tooltip>
		{/if}
		{#if showActions}
			<Tooltip position="bottom" label={m.edit_entity({ entity: m.nickname() })}>
				<CornerDotButton onClick={handleEditNickname} class="h-8 w-8 p-1">
					<Edit />
				</CornerDotButton>
			</Tooltip>
			<Tooltip position="bottom" label={m.max_out_pal_stats(p.pal)}>
				<CornerDotButton
					onClick={() => handleMaxOutPal(pal, appState.selectedPlayer!)}
					class="h-8 w-8 p-1"
				>
					<BicepsFlexed />
				</CornerDotButton>
			</Tooltip>
			<Tooltip position="bottom" label={m.save_as_preset()}>
				<CornerDotButton onClick={handleSavePreset} class="h-8 w-8 p-1">
					<Save />
				</CornerDotButton>
			</Tooltip>
			<Tooltip position="bottom" label={m.apply_preset()}>
				<CornerDotButton onClick={handleSelectPreset} class="h-8 w-8 p-1">
					<Play />
				</CornerDotButton>
			</Tooltip>
		{/if}

		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.gender() })}>
			<CornerDotButton onClick={handleEditGender} class="h-8 w-8 p-1">
				<img
					src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${pal.gender}.webp`)}
					alt={pal.gender}
				/>
			</CornerDotButton>
		</Tooltip>
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.lucky() })}>
			<CornerDotButton
				onClick={handleEditLucky}
				class={cn('h-8 w-8 p-1', pal.is_lucky && 'bg-secondary-500/25')}
				disabled={!showActions}
			>
				<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
			</CornerDotButton>
		</Tooltip>
		<Tooltip position="bottom" label={m.toggle_entity({ entity: m.alpha() })}>
			<CornerDotButton
				onClick={handleEditAlpha}
				class={cn('h-8 w-8 p-1', pal.is_boss && 'bg-secondary-500/25')}
				disabled={!showActions}
			>
				<img
					src={staticIcons.alphaIcon}
					alt="Alpha"
					class="h-8 w-8"
					style="width: 24px; height: 24px;"
				/>
			</CornerDotButton>
		</Tooltip>
		{#if getPalElementTypes(pal.character_key)}
			{#each getPalElementTypes(pal.character_key)! as elementType}
				{#if getPalElementBadge(elementType)}
					<img src={getPalElementBadge(elementType)} alt={elementType} class="h-8 w-8" />
				{/if}
			{/each}
		{/if}
	</div>
</div>