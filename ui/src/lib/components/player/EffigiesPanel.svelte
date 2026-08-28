<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { fade } from 'svelte/transition';
	import { Button, Slider, Spinner, Tooltip } from '$components/ui';
	import { relicData } from '$lib/data';
	import { getToastState } from '$states';
	import { assetLoader } from '$utils';
	import {
		RELIC_ORDER,
		clampCount,
		deriveStatusPatches,
		isIllegalCount,
		relicIconPath,
		relicIndexLabel,
		statKeyFor
	} from '$utils/effigies';
	import { EntryState, type Player } from '$types';
	import * as m from '$i18n/messages';

	let { player = $bindable() }: { player: Player } = $props();

	const toast = getToastState();

	type ViewMode = 'list' | 'grid';
	let viewMode = $state<ViewMode>('grid');

	// Empty until the game-data fetch lands; editors stay disabled without caps.
	let relics = $derived(relicData.relics);

	// A pre-1.0 save carries no RelicPossessNumMap at all: the wire sends the
	// field as JSON `null`, counts read as absent (not zero), and nothing here
	// is editable.
	let supported = $derived(player.relic_possess_num_map != null);

	// Working copy of the counts, re-staged whenever the loaded player changes.
	let values = $state<Record<string, number>>({});
	// Re-stage whenever the loaded player changes (not just when the map does:
	// two players can hold identical maps, and the stale working copy must not
	// follow the select switch).
	let stagedForUid = $state<string>('');
	$effect(() => {
		if (stagedForUid !== player.uid) {
			stagedForUid = player.uid;
			values = { ...(player.relic_possess_num_map ?? {}) };
		}
	});

	let dirty = $derived.by(() => {
		const loaded = player.relic_possess_num_map ?? {};
		const keys = new Set([...Object.keys(values), ...Object.keys(loaded)]);
		for (const key of keys) {
			if ((values[key] ?? 0) !== (loaded[key] ?? 0)) return true;
		}
		return false;
	});

	// The 13 known types in game order, then any type this save carries that
	// the game data does not know -- surfaced read-only rather than dropped.
	let knownRelics = $derived(RELIC_ORDER.filter((key) => relics[key] !== undefined));
	let unknownTypes = $derived(
		Object.keys(player.relic_possess_num_map ?? {})
			.filter((key) => !RELIC_ORDER.includes(key as (typeof RELIC_ORDER)[number]))
			.sort()
	);

	function countOf(relicKey: string): number {
		return values[relicKey] ?? 0;
	}

	function loadedCount(relicKey: string): number {
		return player.relic_possess_num_map?.[relicKey] ?? 0;
	}

	// The SAVED rank (spent effigies), not one derived from the held count --
	// held and spent are independent until Apply syncs them.
	function storedRank(relicKey: string): number {
		return player.status_point_list[statKeyFor(relicKey)] ?? 0;
	}

	function capOf(relicKey: string): number | undefined {
		return relics[relicKey]?.cumulative_max;
	}

	function setAllToMax(): void {
		for (const relicKey of knownRelics) {
			const cap = capOf(relicKey);
			if (cap === undefined) continue;
			values[relicKey] = cap;
		}
		values = { ...values };
	}

	function setCount(relicKey: string, next: number): void {
		const cap = capOf(relicKey);
		if (cap === undefined) return;
		values[relicKey] = clampCount(next, cap);
		values = { ...values };
	}

	function apply(): void {
		const committed = { ...values };
		Object.assign(
			player.status_point_list,
			deriveStatusPatches(committed, player.relic_possess_num_map ?? {}, relics)
		);
		player.relic_possess_num_map = committed;
		player.state = EntryState.MODIFIED;
		toast.add(m.effigies_updated(), undefined, 'success');
	}

	function reset(): void {
		values = { ...(player.relic_possess_num_map ?? {}) };
	}
</script>

{#if Object.keys(relics).length === 0}
	<div class="flex justify-center py-16">
		<Spinner size="size-6" />
	</div>
{:else}
	<div class="space-y-4 p-5">
		<div class="flex flex-wrap items-center justify-between gap-2">
			<p class="text-surface-400 flex items-center gap-1.5 font-semibold tracking-widest uppercase">
				<Icon icon="tabler:diamond" size={20} class="text-secondary-500" />
				{m.edit_effigies()}
				{#if knownRelics.length}
					<span class="text-surface-500 font-normal normal-case">— {knownRelics.length}</span>
				{/if}
			</p>
			<div class="flex items-center gap-1.5">
				<div
					class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 flex items-center gap-0.5 rounded-sm border p-0.5"
				>
					<button
						type="button"
						class="rounded-sm p-1 transition-colors {viewMode === 'grid'
							? 'bg-surface-200 dark:bg-surface-700 text-on-surface'
							: 'text-surface-400 hover:text-on-surface'}"
						onclick={() => (viewMode = 'grid')}
						aria-label={m.effigies_grid_view()}
					>
						<Icon icon="tabler:grid-3x3" size={13} />
					</button>
					<button
						type="button"
						class="rounded-sm p-1 transition-colors {viewMode === 'list'
							? 'bg-surface-200 dark:bg-surface-700 text-on-surface'
							: 'text-surface-400 hover:text-on-surface'}"
						onclick={() => (viewMode = 'list')}
						aria-label={m.effigies_list_view()}
					>
						<Icon icon="tabler:list" size={13} />
					</button>
				</div>
				<div class="bg-surface-200 dark:bg-surface-700 h-4 w-px"></div>
				<Button variant="ghost" onclick={setAllToMax} disabled={!supported} class="!text-xs">
					<Icon icon="tabler:bolt" size={13} class="mr-1" />{m.max_all_abilities()}
				</Button>
			</div>
		</div>

		{#if !supported}
			<p class="text-warning-500 flex items-center gap-1.5 text-xs">
				<Icon icon="tabler:alert-triangle" size={13} />
				{m.effigies_unsupported()}
			</p>
		{/if}

		{#if knownRelics.length === 0}
			<p class="text-surface-400 py-8 text-center text-sm">{m.edit_effigies()}</p>
		{:else if viewMode === 'list'}
			<!-- ─── LIST VIEW ─── -->
			<div class="space-y-1">
				{#each knownRelics as relicKey (relicKey)}
					{@const entry = relics[relicKey]}
					{@const cap = entry.cumulative_max}
					{@const val = countOf(relicKey)}
					{@const illegal = isIllegalCount(loadedCount(relicKey), cap)}
					<div
						transition:fade={{ duration: 120 }}
						class="hover:bg-surface-200/50 dark:hover:bg-surface-800/40 flex items-center gap-2 rounded-sm px-1 py-1.5 transition-colors"
					>
						<Tooltip
							label={`${entry.localized_name}${illegal ? ` — ${m.effigies_over_cap()}` : ''}`}
						>
							<img
								src={assetLoader.loadImage(relicIconPath(relicKey))}
								alt={entry.localized_name}
								class="h-6 w-6 shrink-0 rounded object-contain"
								loading="lazy"
							/>
						</Tooltip>
						<span class="text-surface-400 w-5 shrink-0 font-mono text-xs tabular-nums">
							#{relicIndexLabel(relicKey)}
						</span>
						<div class="min-w-0 flex-1">
							<p class="text-on-surface truncate text-sm leading-tight">
								{entry.localized_name}
								{#if illegal}
									<Icon
										icon="tabler:alert-triangle"
										size={10}
										class="text-warning-500 ml-1 inline"
									/>
								{/if}
							</p>
							<p class="text-surface-400 text-xs leading-tight">
								{m.rank_hint({ rank: storedRank(relicKey), max: entry.max_rank })}
							</p>
						</div>
						<Slider
							value={val}
							max={cap}
							disabled={!supported}
							size="sm"
							showMax
							completeColor="success"
							label={entry.localized_name}
							onchange={(next) => setCount(relicKey, next)}
							class="flex-1"
						/>
					</div>
				{/each}
			</div>
		{:else}
			<!-- ─── GRID VIEW ─── -->
			<div class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
				{#each knownRelics as relicKey (relicKey)}
					{@const entry = relics[relicKey]}
					{@const cap = entry.cumulative_max}
					{@const val = countOf(relicKey)}
					{@const illegal = isIllegalCount(loadedCount(relicKey), cap)}
					<div
						transition:fade={{ duration: 120 }}
						class="preset-outlined-surface-200-800 hover:bg-surface-200/40 dark:hover:bg-surface-800/40 flex flex-col gap-2.5 rounded-sm p-3 transition-colors"
					>
						<div class="flex items-start gap-2.5">
							<img
								src={assetLoader.loadImage(relicIconPath(relicKey))}
								alt={entry.localized_name}
								class="h-9 w-9 shrink-0 rounded-lg object-contain"
								loading="lazy"
							/>
							<div class="min-w-0 flex-1">
								<div class="flex items-center gap-1.5">
									<span class="text-surface-400 font-mono text-xs tabular-nums">
										#{relicIndexLabel(relicKey)}
									</span>
									<p class="text-on-surface truncate text-sm font-medium">
										{entry.localized_name}
										{#if illegal}
											<Icon
												icon="tabler:alert-triangle"
												size={10}
												class="text-warning-500 ml-1 inline"
											/>
										{/if}
									</p>
								</div>
								<p class="text-surface-400 mt-0.5 text-xs leading-tight">
									{m.rank_hint({ rank: storedRank(relicKey), max: entry.max_rank })}
								</p>
							</div>
						</div>
						<Slider
							value={val}
							max={cap}
							disabled={!supported}
							showMax
							completeColor="success"
							label={entry.localized_name}
							onchange={(next) => setCount(relicKey, next)}
						/>
					</div>
				{/each}
			</div>
		{/if}

		{#if unknownTypes.length > 0}
			<p class="text-warning-500 flex items-start gap-1.5 text-xs">
				<Icon icon="tabler:alert-triangle" size={13} class="mt-0.5 shrink-0" />
				{m.effigies_unknown_types({ types: unknownTypes.join(', ') })}
			</p>
		{/if}

		<div class="border-surface-200 dark:border-surface-800 flex items-center gap-2 border-t pt-2">
			<Button variant="primary" onclick={apply} disabled={!supported || !dirty}>
				<Icon icon="tabler:check" size={14} class="mr-1" />{m.apply_effigies()}
			</Button>
			<Button variant="ghost" onclick={reset} disabled={!dirty}>
				<Icon icon="tabler:rotate" size={13} class="mr-1" />{m.reset()}
			</Button>
		</div>
	</div>
{/if}
