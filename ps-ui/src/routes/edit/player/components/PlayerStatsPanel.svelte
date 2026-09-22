<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Progress, Tooltip } from '$components/ui';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import NumberFlow from '@number-flow/svelte';
	import type { ValueChangeDetails as AccordionValueChangeDetails } from '@zag-js/accordion';

	import { PlayerStats, PlayerHealthBadge } from '$components/player';
	import { PlayerPresets } from '$components/presets';
	import { expData } from '$lib/data';
	import { staticIcons } from '$types/icons';
	import type { Player } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	interface Props {
		player: Player;
		maxLevel: number;
		health: number;
		onLevelIncrement: (event: MouseEvent) => void;
		onLevelDecrement: (event: MouseEvent) => void;
		onUpdateNickname: () => void;
	}

	let {
		player = $bindable(),
		maxLevel,
		health = $bindable(),
		onLevelIncrement,
		onLevelDecrement,
		onUpdateNickname
	}: Props = $props();

	let expanded: string[] = $state(['stats']);

	let { levelProgressToNext, levelProgressValue, levelProgressMax } = $derived.by(() => {
		if (player.level >= maxLevel) {
			return { levelProgressToNext: 0, levelProgressValue: 0, levelProgressMax: 1 };
		}
		const nextExp = expData.expData[player.level + 1];
		return {
			levelProgressToNext: nextExp.TotalEXP - player.exp || 0,
			levelProgressValue: nextExp.NextEXP - (nextExp.TotalEXP - player.exp),
			levelProgressMax: nextExp.NextEXP
		};
	});
</script>

<div class="max-h-[calc(100vh-var(--titlebar-h))] min-w-0 overflow-y-auto">
	<div
		id="player-level"
		class="border-l-surface-600 bg-surface-800 mr-2 mb-2 flex rounded-none border-l-2 p-4"
	>
		<div class="mr-4 flex flex-col items-center justify-center rounded-none">
			<div class="flex items-center">
				<Tooltip position="bottom">
					<Button
						variant="ghost"
						size="icon"
						class="mr-4"
						oncontextmenu={(event: MouseEvent) => event.preventDefault()}
						onmousedown={(event: MouseEvent) => onLevelDecrement(event)}
					>
						<Icon icon="tabler:minus" class="text-primary-500" size={16} />
					</Button>
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

				<div class="flex flex-col items-center justify-center">
					<span class="text-surface-400 text-sm font-bold">{m.level().toUpperCase()}</span>
					<span class="text-xl font-bold xl:text-2xl">
						<NumberFlow value={player.level} />
					</span>
				</div>

				<Tooltip position="bottom">
					<Button
						variant="ghost"
						size="icon"
						class="ml-4"
						oncontextmenu={(event: MouseEvent) => event.preventDefault()}
						onmousedown={(event: MouseEvent) => onLevelIncrement(event)}
					>
						<Icon icon="tabler:plus" class="text-primary-500" size={16} />
					</Button>
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
							<span class="text-xs font-bold">Level {maxLevel}</span>
						</div>
					{/snippet}
				</Tooltip>
			</div>
		</div>

		<div class="grow">
			<div class="flex flex-col">
				<div class="flex space-x-2">
					<button
						id="player-nickname"
						class="hover:bg-secondary-500/50 hover:ring-offset-surface-900 text-start font-bold hover:ring hover:ring-offset-4"
						onclick={onUpdateNickname}
					>
						<Icon icon="tabler:edit" class="h-4 w-4" />
					</button>
					<Tooltip label={new Date(player.last_online_time).toLocaleString()}>
						<span class="truncate">{player.nickname}</span>
					</Tooltip>
				</div>
				<div class="flex flex-col space-y-2">
					<div class="flex">
						<span class="text-on-surface grow">NEXT</span>
						<span class="text-on-surface">{levelProgressToNext}</span>
					</div>
					<Progress
						value={levelProgressValue}
						max={levelProgressMax}
						height="h-2"
						width="w-full"
						rounded="rounded-none"
						showLabel={false}
					/>
				</div>
			</div>
		</div>
	</div>
	<PlayerHealthBadge bind:player bind:maxHp={health} />
	<Accordion
		value={expanded}
		onValueChange={(e: AccordionValueChangeDetails) => (expanded = e.value)}
		collapsible
	>
		<Accordion.Item value="stats" controlHover="hover:bg-secondary-500/25">
			{#snippet control()}
				{m.stats()}
			{/snippet}
			{#snippet panel()}
				<PlayerStats {player} />
			{/snippet}
		</Accordion.Item>
		<hr class="hr" />
		<Accordion.Item value="presets" controlHover="hover:bg-secondary-500/25">
			{#snippet control()}
				<div id="player-presets-control" class="w-full">
					{c.preset}
				</div>
			{/snippet}
			{#snippet panel()}
				<PlayerPresets bind:player />
			{/snippet}
		</Accordion.Item>
	</Accordion>
</div>

<style lang="postcss">
	img {
		opacity: 0;
		animation: fadeIn 0.3s ease-in forwards;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	img:not([src]) {
		animation: fadeOut 0.3s ease-out forwards;
	}

	@keyframes fadeOut {
		from {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}
</style>
