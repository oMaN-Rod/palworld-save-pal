<script lang="ts">
	import { TextInputModal } from '$components';
	import { CornerDotButton, Progress, Tooltip } from '$components/ui';
	import { type ElementType, EntryState, type Pal, PalGender, type PresetProfile } from '$types';
	import { ASSET_DATA_PATH, MAX_LEVEL, staticIcons } from '$lib/constants';
	import { palsData, elementsData, expData, presetsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Rating } from '@skeletonlabs/skeleton-svelte';
	import { Minus, Plus } from 'lucide-svelte';
	import { assetLoader, handleMaxOutPal, canBeBoss } from '$utils';

	let { pal = $bindable(), showActions = true } = $props<{
		pal?: Pal;
		showActions?: boolean;
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
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`);
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
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Add Pal preset',
			value: ''
		});
		if (!result) return;

		const newPreset = {
			name: result,
			type: 'pal_preset',
			pal_preset: {
				is_lucky: pal.is_lucky,
				is_boss: pal.is_boss,
				gender: pal.gender,
				rank_hp: pal.rank_hp,
				rank_attack: pal.rank_attack,
				rank_defense: pal.rank_defense,
				rank_craftspeed: pal.rank_craftspeed,
				talent_hp: pal.talent_hp,
				talent_shot: pal.talent_shot,
				talent_defense: pal.talent_defense,
				rank: pal.rank,
				level: pal.level,
				learned_skills: pal.learned_skills,
				active_skills: pal.active_skills,
				passive_skills: pal.passive_skills,
				work_suitability: pal.work_suitability,
				sanity: pal.sanity,
				exp: pal.exp
			}
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
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
				<div class="flex flex-row items-center space-x-2">
					<h6 class="h6 grow">
						{pal.nickname || pal.name}
					</h6>
					{#if showActions}
						<Tooltip position="bottom" label="Edit nickname">
							<CornerDotButton label="Edit" onClick={handleEditNickname} />
						</Tooltip>
						<Tooltip position="bottom" label="Max out Pal stats 💉💪">
							<CornerDotButton
								label="Max"
								onClick={() => handleMaxOutPal(pal, appState.selectedPlayer!)}
							/>
						</Tooltip>
						<Tooltip position="bottom" label="Save as preset">
							<CornerDotButton label="Save" onClick={handleSavePreset} />
						</Tooltip>
					{/if}

					<Tooltip position="bottom">
						<button
							class="hover:ring-secondary-500 relative flex h-full w-auto items-center justify-center hover:ring"
							onclick={handleEditGender}
							disabled={!showActions}
						>
							<div class="h-8 w-8">
								<img
									src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/${pal.gender}.png`)}
									alt={pal.gender}
								/>
							</div>
							<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
						</button>
						{#snippet popup()}
							<span>Toggle gender</span>
						{/snippet}
					</Tooltip>
					<Tooltip position="bottom">
						<button
							class={cn(
								'hover:ring-secondary-500 relative flex h-full w-auto items-center justify-center hover:ring',
								pal.is_lucky && 'bg-secondary-500/25'
							)}
							onclick={handleEditLucky}
							disabled={!showActions}
						>
							<div class="flex h-8 w-8 items-center justify-center">
								<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
							</div>
							<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
						</button>
						{#snippet popup()}
							<span>Toggle Lucky</span>
						{/snippet}
					</Tooltip>
					<Tooltip position="bottom">
						<button
							class={cn(
								'hover:ring-secondary-500 relative flex h-full w-auto items-center justify-center hover:ring',
								pal.is_boss && 'bg-secondary-500/25'
							)}
							onclick={handleEditAlpha}
							disabled={!showActions}
						>
							<div class="flex h-8 w-8 items-center justify-center">
								<img
									src={staticIcons.alphaIcon}
									alt="Alpha"
									class="h-8 w-8"
									style="width: 24px; height: 24px;"
								/>
							</div>
							<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
							<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
						</button>
						{#snippet popup()}
							<span>Toggle Alpha</span>
						{/snippet}
					</Tooltip>
					<div class="flex flex-row items-center">
						<div class="flex flex-row">
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
