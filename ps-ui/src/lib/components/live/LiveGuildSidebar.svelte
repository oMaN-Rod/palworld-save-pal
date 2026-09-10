<script lang="ts">
	import { Button, Input, Popover, Spinner, Tooltip } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { cn } from '$theme';
	import type { LiveGuildView } from './liveGuild.utils';
	import type { GameGuildSummaryJson } from '$states/gameState.svelte';
	import * as m from '$i18n/messages';

	let {
		name = null,
		guildChoices = [],
		selectedGuildId = null,
		onSelectGuild,
		baseCount = null,
		memberCount = 0,
		baseCampLevel = null,
		levelBusy = false,
		setLevelReason,
		onSetBaseCampLevel,
		counts = {},
		activeView,
		onSelectView
	}: {
		name?: string | null;
		guildChoices?: GameGuildSummaryJson[];
		selectedGuildId?: string | null;
		onSelectGuild?: (guildId: string) => void;
		baseCount?: number | null;
		memberCount?: number;
		baseCampLevel?: number | null;
		levelBusy?: boolean;
		setLevelReason?: string;
		onSetBaseCampLevel?: (level: number) => void;
		counts?: Partial<Record<LiveGuildView, string>>;
		activeView: LiveGuildView;
		onSelectView: (view: LiveGuildView) => void;
	} = $props();

	const MIN_BASE_CAMP_LEVEL = 1;
	const MAX_BASE_CAMP_LEVEL = 50;

	const ENTRIES: { view: LiveGuildView; icon: string; label: () => string }[] = [
		{ view: 'members', icon: 'tabler:users', label: m.live_guild_view_members },
		{ view: 'lab', icon: 'tabler:flask', label: m.live_guild_view_lab },
		{ view: 'pals', icon: 'tabler:paw', label: m.live_guild_view_base_pals },
		{ view: 'storage', icon: 'tabler:box', label: m.live_guild_view_storage },
		{ view: 'chest', icon: 'tabler:diamond', label: m.live_guild_view_chest }
	];

	let levelDraft = $state<number | null>(null);
	let levelMessage = $state('');
	let seededFrom = $state<number | null>(null);

	$effect(() => {
		if (baseCampLevel !== null && baseCampLevel !== seededFrom) {
			seededFrom = baseCampLevel;
			levelDraft = baseCampLevel;
		}
	});

	function submitLevel(): void {
		const level = levelDraft;
		if (
			level === null ||
			!Number.isInteger(level) ||
			level < MIN_BASE_CAMP_LEVEL ||
			level > MAX_BASE_CAMP_LEVEL
		) {
			levelMessage = m.live_level_out_of_range({
				min: MIN_BASE_CAMP_LEVEL,
				max: MAX_BASE_CAMP_LEVEL
			});
			return;
		}
		levelMessage = '';
		onSetBaseCampLevel?.(level);
	}
</script>

<div
	id="live-guild-sidebar"
	class="border-surface-800 bg-surface-900 flex w-13 shrink-0 flex-col border-r @min-[700px]/guild:w-54"
>
	<div class="border-surface-800 flex justify-center border-b p-2 @min-[700px]/guild:hidden">
		<Popover position="right-start" popoverClass="w-64">
			<button
				type="button"
				id="live-guild-settings-trigger"
				aria-label={m.live_guild_settings()}
				class="hover:bg-surface-800 flex h-9 w-9 items-center justify-center rounded-sm"
			>
				<Icon icon="tabler:settings" class="h-4 w-4" />
			</button>

			{#snippet content()}
				<div class="flex flex-col gap-2">
					{@render guildIdentity('live-guild-settings')}
					{@render levelStepper('live-guild-settings')}
				</div>
			{/snippet}
		</Popover>
	</div>

	<div class="border-surface-800 hidden flex-col gap-2 border-b p-3 @min-[700px]/guild:flex">
		{@render guildIdentity('live')}

		<div class="flex gap-4">
			<div class="flex flex-col">
				<span class="text-surface-400 text-xs">{m.live_guild_bases()}</span>
				<span class="text-sm tabular-nums">{baseCount ?? '—'}</span>
			</div>
			<div class="flex flex-col">
				<span class="text-surface-400 text-xs">{m.live_guild_members()}</span>
				<span class="text-sm tabular-nums">{memberCount}</span>
			</div>
		</div>

		{@render levelStepper('live')}
	</div>

	<nav class="flex flex-col gap-0.5 p-2">
		{#each ENTRIES as entry (entry.view)}
			<button
				type="button"
				class={cn(
					'hover:bg-surface-800 flex items-center gap-2 rounded-sm border-l-2 border-transparent px-2 py-1.5 text-left text-sm',
					activeView === entry.view && 'bg-secondary-800/40 border-l-secondary-400'
				)}
				onclick={() => onSelectView(entry.view)}
			>
				<Icon icon={entry.icon} class="h-4 w-4 shrink-0" />
				<span class="sr-only truncate @min-[700px]/guild:not-sr-only">{entry.label()}</span>
				{#if counts[entry.view]}
					<span
						class="bg-surface-800 text-surface-400 sr-only ml-auto rounded-full px-1.5 text-xs tabular-nums @min-[700px]/guild:not-sr-only"
					>
						{counts[entry.view]}
					</span>
				{/if}
			</button>
		{/each}
	</nav>
</div>

{#snippet guildIdentity(idPrefix: string)}
	<span class="truncate text-base font-semibold">{name ?? '—'}</span>

	{#if guildChoices.length > 1}
		<select
			id="{idPrefix}-guild-picker"
			aria-label={m.live_switch_guild()}
			class="select w-full text-xs"
			value={selectedGuildId ?? ''}
			onchange={(event: Event) =>
				onSelectGuild?.((event.currentTarget as HTMLSelectElement).value)}
		>
			{#each guildChoices as choice (choice.id)}
				<option value={choice.id}>
					{choice.name || choice.id}{choice.members ? ` (${choice.members.length})` : ''}
				</option>
			{/each}
		</select>
	{/if}
{/snippet}

{#snippet levelStepper(idPrefix: string)}
	<label class="text-surface-400 text-xs" for="{idPrefix}-guild-level">
		{m.live_guild_base_camp_level()}
	</label>
	<div class="flex items-center gap-2">
		<Input
			id="{idPrefix}-guild-level"
			type="number"
			inputClass="w-20"
			value={levelDraft ?? ''}
			disabled={levelBusy}
			oninput={(event: Event) => {
				const raw = (event.currentTarget as HTMLInputElement).value;
				levelDraft = raw === '' ? null : Number(raw);
			}}
		/>
		<Tooltip label={setLevelReason ?? m.live_set_level()}>
			<Button size="sm" disabled={levelBusy || !!setLevelReason} onclick={submitLevel}>
				{m.live_set_level()}
			</Button>
		</Tooltip>
		{#if levelBusy}
			<Spinner size="size-5" />
		{/if}
	</div>
	{#if levelMessage}
		<p class="text-error-400 text-xs">{levelMessage}</p>
	{/if}
{/snippet}
