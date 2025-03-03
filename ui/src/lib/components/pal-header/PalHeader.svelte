<script lang="ts">
	import { DebugModal, PresetConfigModal, TextInputModal } from '$components';
	import { CornerDotButton, Progress, Tooltip } from '$components/ui';
	import {
		defaultPresetConfig,
		type ElementType,
		EntryState,
		type Pal,
		PalGender,
		type PalPresetConfig,
		type PresetProfile
	} from '$types';
	import { ASSET_DATA_PATH, MAX_LEVEL, staticIcons } from '$lib/constants';
	import { palsData, elementsData, expData, presetsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Rating } from '@skeletonlabs/skeleton-svelte';
	import { BicepsFlexed, Bug, Edit, Minus, Plus, Save } from 'lucide-svelte';
	import { assetLoader, handleMaxOutPal, canBeBoss } from '$utils';
	import { goto } from '$app/navigation';

	let {
		pal = $bindable(),
		showActions = true,
		popup = false
	} = $props<{
		pal?: Pal;
		showActions?: boolean;
		popup?: boolean;
	}>();

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	let palLevelProgressToNext: number = $state(0);
	let palLevelProgressValue: number = $state(0);
	let palLevelProgressMax: number = $state(1);

	const palLevel = $derived.by(() => {
		if (appState.selectedPlayer && pal) {
			return appState.selectedPlayer.level < pal.level
				? appState.selectedPlayer.level.toString()
				: pal.level.toString();
		}
	});
	const palLevelClass = $derived.by(() => {
		if (appState.selectedPlayer && pal) {
			return appState.selectedPlayer.level < pal.level ? 'text-error-500' : '';
		}
	});
	const palLevelMessage = $derived.by(() => {
		if (appState.selectedPlayer && pal) {
			return appState.selectedPlayer.level < pal.level ? 'Level sync' : 'No Level Sync';
		}
	});
	const palRank = $derived(pal ? pal.rank - 1 : 0);

	async function calcPalLevelProgress() {
		if (pal) {
			if (pal.level === 60) {
				palLevelProgressToNext = 0;
				palLevelProgressValue = 0;
				palLevelProgressMax = 1;
				return;
			}
			const nextExp = await expData.getExpDataByLevel(pal.level + 1);
			palLevelProgressToNext = nextExp.PalTotalEXP - pal.exp;
			palLevelProgressValue = nextExp.PalNextEXP - palLevelProgressToNext;
			palLevelProgressMax = nextExp.PalNextEXP;
		}
	}

	async function handleLevelIncrement() {
		if (!pal || !appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const newLevel = Math.min(pal.level + 1, MAX_LEVEL);
		if (newLevel === pal.level) return;

		const nextLevelData = await expData.getExpDataByLevel(newLevel + 1);

		pal.level = newLevel;
		pal.exp = nextLevelData.PalTotalEXP - nextLevelData.PalNextEXP;
		pal.state = EntryState.MODIFIED;

		await calcPalLevelProgress();
	}

	async function handleLevelDecrement() {
		if (!pal || !appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const newLevel = Math.max(pal.level - 1, 1);
		if (newLevel === pal.level) return;

		const newLevelData = await expData.getExpDataByLevel(newLevel + 1);

		pal.level = newLevel;
		pal.exp = newLevelData.PalTotalEXP - newLevelData.PalNextEXP;
		pal.state = EntryState.MODIFIED;

		await calcPalLevelProgress();
	}

	async function getPalElementTypes(character_id: string): Promise<ElementType[] | undefined> {
		const palData = palsData.pals[character_id];
		if (!palData) return undefined;
		return palData.element_types.length > 0 ? palData.element_types : undefined;
	}

	async function getPalElementBadge(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		if (!elementObj) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.png`);
	}

	async function handleEditNickname() {
		if (!pal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit nickname',
			value: pal.nickname || pal.name
		});
		if (!result) return;
		pal.nickname = result;
		pal.state = EntryState.MODIFIED;
		if (appState.selectedPlayer && appState.selectedPlayer.pals)
			appState.selectedPlayer.pals[pal.instance_id].nickname = result;
	}

	function handleEditGender() {
		if (pal) {
			const currentGender = pal.gender;
			pal.gender = currentGender === PalGender.MALE ? PalGender.FEMALE : PalGender.MALE;
			pal.state = EntryState.MODIFIED;
		}
	}

	function handleEditLucky() {
		const [type, valid] = canBeBoss(pal.character_id);
		if (!valid) {
			toast.add(`${type} Pal cannot be Lucky`, undefined, 'warning');
			return;
		}
		if (pal) {
			pal.is_lucky = !pal.is_lucky;
			pal.is_boss = pal.is_lucky ? false : pal.is_boss;
			pal.state = EntryState.MODIFIED;
		}
	}

	function handleEditAlpha() {
		const [type, valid] = canBeBoss(pal.character_id);
		if (!valid) {
			toast.add(`${type} Pal cannot be Alpha`, undefined, 'warning');
			return;
		}
		if (pal) {
			pal.is_boss = !pal.is_boss;
			pal.is_lucky = pal.is_boss ? false : pal.is_lucky;
			pal.state = EntryState.MODIFIED;
		}
	}

	$effect(() => {
		calcPalLevelProgress();
	});

	async function handleSavePreset() {
		// @ts-ignore
		const result = await modal.showModal(PresetConfigModal, {
			config: defaultPresetConfig,
			characterId: pal.name
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
				exp: config.exp ? pal.exp : null
			}
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
	}

	async function handleDebugPal() {
		// @ts-ignore
		await modal.showModal(DebugModal, {
			title: 'Pal Debug',
			json: { content: { text: JSON.stringify(pal, null, 2) } }
		});
	}
</script>

{#if pal}
	<div
		class="border-l-surface-600 preset-filled-surface-100-900 flex flex-row rounded-none border-l-2 p-4"
	>
		<div class="mr-4 flex flex-col items-center justify-center rounded-none">
			<Rating
				value={palRank}
				count={4}
				itemClasses="text-gray"
				onValueChange={(e) => {
					pal.rank = e.value + 1;
					pal.state = EntryState.MODIFIED;
				}}
			/>
			<div class="flex flex-row px-2">
				{#if showActions}
					<button class="mr-4">
						<Minus class="text-primary-500" onclick={handleLevelDecrement} />
					</button>
				{/if}

				<Tooltip>
					<div class="flex flex-col items-center justify-center">
						<span class={cn('text-surface-400 font-bold', palLevelClass)}>LEVEL</span>
						<span class={cn('text-4xl font-bold', palLevelClass)}>{palLevel}</span>
					</div>
					{#snippet popup()}
						{palLevelMessage}
					{/snippet}
				</Tooltip>

				{#if showActions}
					<button class="ml-4">
						<Plus class="text-primary-500" onclick={handleLevelIncrement} />
					</button>
				{/if}
			</div>
		</div>

		<div class="grow">
			<div class="flex flex-col">
				<div
					class={cn(
						'flex flex-col items-start space-y-2 2xl:flex-row 2xl:space-x-2 2xl:space-y-0',
						popup ? '2xl:flex-col 2xl:space-y-0' : ''
					)}
				>
					<h6 class="h6 grow">
						{pal.nickname || pal.name}
					</h6>
					<div class="flex space-x-2">
						{#if appState.settings.debug_mode}
							<Tooltip position="bottom" label="Debug">
								<CornerDotButton
									onClick={() => {
										goto(
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
							<Tooltip position="bottom" label="Edit nickname">
								<CornerDotButton onClick={handleEditNickname} class="h-8 w-8 p-1">
									<Edit />
								</CornerDotButton>
							</Tooltip>
							<Tooltip position="bottom" label="Max out Pal stats 💉💪">
								<CornerDotButton
									onClick={() => handleMaxOutPal(pal, appState.selectedPlayer!)}
									class="h-8 w-8 p-1"
								>
									<BicepsFlexed />
								</CornerDotButton>
							</Tooltip>
							<Tooltip position="bottom" label="Save as preset">
								<CornerDotButton onClick={handleSavePreset} class="h-8 w-8 p-1">
									<Save />
								</CornerDotButton>
							</Tooltip>
						{/if}

						<Tooltip position="bottom" label="Toggle gender">
							<CornerDotButton onClick={handleEditGender} class="h-8 w-8 p-1">
								<img
									src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${pal.gender}.png`)}
									alt={pal.gender}
								/>
							</CornerDotButton>
						</Tooltip>
						<Tooltip position="bottom" label="Toggle Lucky">
							<CornerDotButton
								onClick={handleEditLucky}
								class={cn('h-8 w-8 p-1', pal.is_lucky && 'bg-secondary-500/25')}
								disabled={!showActions}
							>
								<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
							</CornerDotButton>
						</Tooltip>
						<Tooltip position="bottom" label="Toggle Alpha">
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
						{#await getPalElementTypes(pal.character_key) then elementTypes}
							{#if elementTypes}
								{#each elementTypes as elementType}
									{#await getPalElementBadge(elementType) then icon}
										<img src={icon} alt={elementType} class="h-8 w-8" />
									{/await}
								{/each}
							{/if}
						{/await}
					</div>
				</div>
				<hr class="hr my-1" />
				<div class="flex flex-col space-y-2">
					<div class="flex">
						<span class="text-on-surface grow">NEXT</span>
						<span class="text-on-surface">{palLevelProgressToNext}</span>
					</div>
					<Progress
						bind:value={palLevelProgressValue}
						bind:max={palLevelProgressMax}
						height="h-2"
						width="w-full"
						rounded="rounded-none"
						showLabel={false}
					/>
				</div>
			</div>
		</div>
	</div>
{/if}
