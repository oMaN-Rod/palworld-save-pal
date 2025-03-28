<script lang="ts">
	import { PalPresetSelectModal, PresetConfigModal, TextInputModal } from '$components';
	import { CornerDotButton, Progress, Tooltip, Input } from '$components/ui';
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
	import { palsData, elementsData, expData, presetsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Rating } from '@skeletonlabs/skeleton-svelte';
	import { BicepsFlexed, Bug, Edit, Minus, Play, Plus, Save } from 'lucide-svelte';
	import { assetLoader, handleMaxOutPal, canBeBoss } from '$utils';
	import { goto } from '$app/navigation';
	import { staticIcons } from '$types/icons';
	import { valueType } from 'svelte-jsoneditor';

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

	const max_level = $derived(appState.settings.cheat_mode ? 255 : 60)
	const max_rank = $derived(appState.settings.cheat_mode ? 255 : 5)

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

	async function handleLevelIncrement(event: MouseEvent) {
		if (!pal || !appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		let newLevel = pal.level

		if (event.ctrlKey) {
			if (event.button === 0) {
				newLevel = Math.min(pal.level + 5, max_level);
			} else if (event.button === 1) {
				newLevel = max_level
			} else if (event.button === 2) {
				newLevel = Math.min(pal.level + 10, max_level);
			}
		} else {
			newLevel = Math.min(pal.level + 1, max_level);
		}

		if (newLevel === pal.level) return;

		const nextLevelData = await expData.getExpDataByLevel(newLevel + 1);

		pal.level = newLevel;
		pal.exp = nextLevelData.PalTotalEXP - nextLevelData.PalNextEXP;
		pal.state = EntryState.MODIFIED;

		await calcPalLevelProgress();
	}

	async function handleLevelDecrement(event: MouseEvent) {
		if (!pal || !appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		let newLevel = pal.level

		if (event.ctrlKey) {
			if (event.button === 0) {
				newLevel = Math.max(pal.level - 5, 1);
			} else if (event.button === 1) {
				newLevel = 1
			} else if (event.button === 2) {
				newLevel = Math.max(pal.level - 10, 1);
			}
		} else {
			newLevel = Math.max(pal.level - 1, 1);
		}

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
			formatBossCharacterId();
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
			formatBossCharacterId();
			pal.state = EntryState.MODIFIED;
		}
	}

	function formatBossCharacterId() {
		if (pal && (pal.is_boss || pal.is_lucky) && !pal.character_id.startsWith('BOSS_')) {
			pal.character_id = `BOSS_${pal.character_id}`;
		} else if (pal && !pal.is_boss && !pal.is_lucky && pal.character_id.startsWith('BOSS_')) {
			pal.character_id = pal.character_id.replace('BOSS_', '');
		}
	}

	$effect(() => {
		calcPalLevelProgress();
	});

	async function handleSelectPreset() {
		// @ts-ignore
		const result = await modal.showModal<string>(PalPresetSelectModal, {
			title: 'Select preset',
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
				pal.is_boss = true
				pal.is_lucky = false
			}
			if (key === 'is_lucky' && value && pal.is_boss) {
				pal.is_boss = false
				pal.is_lucky = true
			}
			else if (value !== null) {
				(pal as Record<string, any>)[key] = value;
			}
		}
		pal.state = EntryState.MODIFIED;
	}

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

	async function handleInputUpdate(value: number) {
		pal.rank = value
		pal.state = EntryState.MODIFIED
	}
</script>

{#if pal}
	<div
		class="border-l-surface-600 preset-filled-surface-100-900 flex flex-row rounded-none border-l-2 p-4"
	>
		<div class="mr-4 flex flex-col items-center justify-center rounded-none">
			{#if appState.settings.cheat_mode}
				<Input 
					bind:value={pal.rank} 
					placeholder="Rank" 
					type="number"
					itemClasses="text-gray"
					min={0}
					max={max_rank}
					onchange={handleInputUpdate}
				/>
			{:else}
				<Rating
					value={palRank}
					count={4}
					itemClasses="text-gray"
					onValueChange={(e) => {
						pal.rank = e.value + 1;
						pal.state = EntryState.MODIFIED;
					}}
				/>
			{/if}
			<div class="flex flex-row px-2">
				{#if showActions}
					<Tooltip position='bottom'>
						<button 
							oncontextmenu={(event) => event.preventDefault()}
							class="mr-4 hover:bg-secondary-500/25"
							onmousedown={(event) => handleLevelDecrement(event)}
						>
							<Minus class="text-primary-500" />
						</button>
						{#snippet popup()}
						<div class="flex items-center space-x-2">
							<div class="h-6 w-6">
								<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
							</div>
							<div class="h-6 w-6">
								<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-full w-full" />
							</div>
							<span class="text-xs font-bold">-5</span>
						</div>
						<div class="flex items-center space-x-2">
							<div class="h-6 w-6">
								<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
							</div>
							<div class="h-6 w-6">
								<img src={staticIcons.rightClickIcon} alt="Right Click" class="h-full w-full" />
							</div>
							<span class="text-xs font-bold">-10</span>
						</div>
						<div class="flex items-center space-x-2">
							<div class="h-6 w-6">
								<img src={staticIcons.ctrlIcon} alt="Right Click" class="h-full w-full" />
							</div>
							<div class="h-6 w-6">
								<img src={staticIcons.middleClickIcon} alt="Middle Click" class="h-full w-full" />
							</div>
							<span class="text-xs font-bold">Level 1</span>
						</div>
						{/snippet}
					</Tooltip>
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
					<Tooltip position='bottom'>
						<button
							oncontextmenu={(event) => event.preventDefault()}
							class="ml-4 hover:bg-secondary-500/25"
							onmousedown={(event) => handleLevelIncrement(event)}
						>
							<Plus class="text-primary-500" />
						</button>
						{#snippet popup()}
						<div class="flex items-center space-x-2">
							<div class="h-6 w-6">
								<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
							</div>
							<div class="h-6 w-6">
								<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-full w-full" />
							</div>
							<span class="text-xs font-bold">+5</span>
						</div>
						<div class="flex items-center space-x-2">
							<div class="h-6 w-6">
								<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
							</div>
							<div class="h-6 w-6">
								<img src={staticIcons.rightClickIcon} alt="Right Click" class="h-full w-full" />
							</div>
							<span class="text-xs font-bold">+10</span>
						</div>
						<div class="flex items-center space-x-2">
							<div class="h-6 w-6">
								<img src={staticIcons.ctrlIcon} alt="Right Click" class="h-full w-full" />
							</div>
							<div class="h-6 w-6">
								<img src={staticIcons.middleClickIcon} alt="Middle Click" class="h-full w-full" />
							</div>
							<span class="text-xs font-bold">Level {max_level}</span>
						</div>
						{/snippet}
					</Tooltip>
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
							<Tooltip position="bottom" label="Apply a preset">
								<CornerDotButton onClick={handleSelectPreset} class="h-8 w-8 p-1">
									<Play />
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
