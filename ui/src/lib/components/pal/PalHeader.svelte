<script lang="ts">
	import { Button, CornerDotButton, Progress, Tooltip, Input } from '$components/ui';
	import { EntryState, type Pal } from '$types';
	import { palsData, expData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState } from '$states';
	import { Rating } from '@skeletonlabs/skeleton-svelte';
	import { Minus, Plus } from 'lucide-svelte';
	import { staticIcons } from '$types/icons';
	import NumberFlow from '@number-flow/svelte';
	import type { ValueChangeDetails } from '@zag-js/rating-group';
	import * as m from '$i18n/messages';
	import PalActionButtons from './PalActionButtons.svelte';
	import { MAX_LEVEL } from '$lib/constants';

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

	const max_level = $derived(appState.settings.cheat_mode ? 255 : MAX_LEVEL);
	const max_rank = $derived(appState.settings.cheat_mode ? 255 : 5);

	let palLevelProgressToNext: number = $state(0);
	let palLevelProgressValue: number = $state(0);
	let palLevelProgressMax: number = $state(1);

	const palLevel = $derived.by(() => {
		if (appState.selectedPlayer && pal) {
			return appState.selectedPlayer.level < pal.level
				? appState.selectedPlayer.level.toString()
				: pal.level.toString();
		} else if (pal) {
			return pal.level.toString();
		}
	});
	const palLevelClass = $derived.by(() => {
		if (appState.selectedPlayer && pal) {
			return appState.selectedPlayer.level < pal.level ? 'text-error-500' : '';
		}
	});
	const palLevelMessage = $derived.by(() => {
		if (appState.selectedPlayer && pal) {
			return appState.selectedPlayer.level < pal.level
				? `Level sync ${pal.level} → ${appState.selectedPlayer.level}`
				: 'No Level Sync';
		}
	});
	const palRank = $derived(pal ? pal.rank - 1 : 0);

	async function calcPalLevelProgress() {
		if (pal) {
			if (pal.level === max_level) {
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
		if (!pal) return;

		let newLevel = pal.level;

		if (event.ctrlKey) {
			if (event.button === 0) {
				newLevel = Math.min(pal.level + 5, max_level);
			} else if (event.button === 1) {
				newLevel = max_level;
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
		if (!pal) return;

		let newLevel = pal.level;

		if (event.ctrlKey) {
			if (event.button === 0) {
				newLevel = Math.max(pal.level - 5, 1);
			} else if (event.button === 1) {
				newLevel = 1;
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

	async function handleInputUpdate(value: number) {
		pal.rank = value;
		pal.state = EntryState.MODIFIED;
	}

	$effect(() => {
		calcPalLevelProgress();
	});
</script>

{#if pal}
	<div
		class="border-l-surface-600 bg-surface-800 flex flex-row rounded-none border-l-2 p-4"
	>
		<div class="mr-4 flex flex-col items-center justify-center rounded-none">
			{#if appState.settings.cheat_mode}
				<Input
					value={palRank}
					placeholder="Rank"
					type="number"
					itemClasses="text-gray"
					min={0}
					max={max_rank}
					onValueChange={handleInputUpdate}
				/>
			{:else}
				<Rating
					value={palRank}
					count={4}
					itemClasses="text-gray"
					onValueChange={(e: ValueChangeDetails) => {
						pal.rank = e.value + 1;
						pal.state = EntryState.MODIFIED;
					}}
				/>
			{/if}
			<div class="flex flex-row px-2">
				{#if showActions}
					<Tooltip position="bottom">
						<Button
							variant="ghost"
							size="icon"
							oncontextmenu={(event: MouseEvent) => event.preventDefault()}
							class="mr-4"
							onmousedown={(event: MouseEvent) => handleLevelDecrement(event)}
						>
							<Minus class="text-primary-500" />
						</Button>
						{#snippet popup()}
							<div class="flex gap-2">
								<span>Lvl</span>
								<NumberFlow value={pal.level} />
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
								</div>
								<div class="h-6 w-6">
									<img
										src={staticIcons.leftClickIcon}
										alt="Left Click"
										class="h-full w-full"
									/>
								</div>
								<span class="text-xs font-bold">-5</span>
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
								</div>
								<div class="h-6 w-6">
									<img
										src={staticIcons.rightClickIcon}
										alt="Right Click"
										class="h-full w-full"
									/>
								</div>
								<span class="text-xs font-bold">-10</span>
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									<img
										src={staticIcons.ctrlIcon}
										alt="Right Click"
										class="h-full w-full"
									/>
								</div>
								<div class="h-6 w-6">
									<img
										src={staticIcons.middleClickIcon}
										alt="Middle Click"
										class="h-full w-full"
									/>
								</div>
								<span class="text-xs font-bold">Level 1</span>
							</div>
						{/snippet}
					</Tooltip>
				{/if}

				<Tooltip>
					<div class="flex flex-col items-center justify-center">
						<span class={cn('text-surface-400 font-bold', palLevelClass)}>LEVEL</span>
						<span class={cn('text-4xl font-bold', palLevelClass)}>
							<NumberFlow value={palLevel} />
						</span>
					</div>
					{#snippet popup()}
						{palLevelMessage}
					{/snippet}
				</Tooltip>

				{#if showActions}
					<Tooltip position="bottom">
						<Button
							variant="ghost"
							size="icon"
							oncontextmenu={(event: MouseEvent) => event.preventDefault()}
							class="ml-4"
							onmousedown={(event: MouseEvent) => handleLevelIncrement(event)}
						>
							<Plus class="text-primary-500" />
						</Button>
						{#snippet popup()}
							<div class="flex gap-2">
								<span>Lvl</span>
								<NumberFlow value={pal.level} />
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
								</div>
								<div class="h-6 w-6">
									<img
										src={staticIcons.leftClickIcon}
										alt="Left Click"
										class="h-full w-full"
									/>
								</div>
								<span class="text-xs font-bold">+5</span>
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									<img src={staticIcons.ctrlIcon} alt="Control" class="h-full w-full" />
								</div>
								<div class="h-6 w-6">
									<img
										src={staticIcons.rightClickIcon}
										alt="Right Click"
										class="h-full w-full"
									/>
								</div>
								<span class="text-xs font-bold">+10</span>
							</div>
							<div class="flex items-center space-x-2">
								<div class="h-6 w-6">
									<img
										src={staticIcons.ctrlIcon}
										alt="Right Click"
										class="h-full w-full"
									/>
								</div>
								<div class="h-6 w-6">
									<img
										src={staticIcons.middleClickIcon}
										alt="Middle Click"
										class="h-full w-full"
									/>
								</div>
								<span class="text-xs font-bold">{m.level({ level: max_level })}</span>
							</div>
						{/snippet}
					</Tooltip>
				{/if}
			</div>
		</div>

		<div class="min-w-0 grow">
			<div class="flex flex-col">
				<PalActionButtons bind:pal {showActions} {popup} />
				<hr class="hr my-1" />
				<div class="flex flex-col space-y-2">
					<div class="flex">
						<span class="text-on-surface grow">{m.next()}</span>
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