<script lang="ts">
	// Effigies editor, ported from PalSavTools' Player Editor panel: the
	// progress bar IS the value editor -- click anywhere on it to set the value
	// proportionally, −/+ for ±1 fine adjustment, /max after. Edits stage in a
	// working copy; Apply commits them to the player DTO (counts into
	// relic_possess_num_map, ranks derived into status_point_list) the way every
	// PSP editor defers persistence to the global save.
	import { fade } from 'svelte/transition';
	import { Button, Spinner, Tooltip } from '$components/ui';
	import { relicData } from '$lib/data';
	import type { RelicRankData } from '$lib/data/relic.svelte';
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
	import Gem from '@lucide/svelte/icons/gem';
	import Grid3x3 from '@lucide/svelte/icons/grid-3x3';
	import List from '@lucide/svelte/icons/list';
	import Zap from '@lucide/svelte/icons/zap';
	import Check from '@lucide/svelte/icons/check';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import * as m from '$i18n/messages';

	let { player = $bindable() }: { player: Player } = $props();

	const toast = getToastState();

	type ViewMode = 'list' | 'grid';
	let viewMode = $state<ViewMode>('grid');

	// Empty until the game-data fetch lands; editors stay disabled without caps.
	let relics = $state<Record<string, RelicRankData>>({});
	$effect(() => {
		relicData
			.getRelicData()
			.then((data) => (relics = data))
			.catch((error) => console.error('Failed to load relic data; effigy editing disabled', error));
	});

	// A pre-1.0 save carries no RelicPossessNumMap at all: counts read as
	// absent, not zero, and nothing here is editable.
	let supported = $derived(player.relic_possess_num_map !== undefined);

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

	function bump(relicKey: string, delta: number): void {
		const cap = capOf(relicKey);
		if (cap === undefined) return;
		values[relicKey] = clampCount(countOf(relicKey) + delta, cap);
		values = { ...values };
	}

	function barClick(relicKey: string, e: MouseEvent): void {
		const cap = capOf(relicKey);
		if (cap === undefined) return;
		const bar = e.currentTarget as HTMLElement;
		const rect = bar.getBoundingClientRect();
		const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		values[relicKey] = Math.round(pct * cap);
		values = { ...values };
	}

	// The bar carries role="slider", so it answers the keys a slider should:
	// arrows step ±1 (±5% with Shift, coarser on the wide move_speed bar),
	// Home/End snap to the ends.
	function barKeydown(relicKey: string, e: KeyboardEvent): void {
		const cap = capOf(relicKey);
		if (cap === undefined) return;
		const step = Math.max(1, Math.round(cap / 20));
		let next: number | undefined;
		switch (e.key) {
			case 'ArrowLeft':
			case 'ArrowDown':
				next = clampCount(countOf(relicKey) - (e.shiftKey ? step : 1), cap);
				break;
			case 'ArrowRight':
			case 'ArrowUp':
				next = clampCount(countOf(relicKey) + (e.shiftKey ? step : 1), cap);
				break;
			case 'Home':
				next = 0;
				break;
			case 'End':
				next = cap;
				break;
		}
		if (next === undefined) return;
		e.preventDefault();
		values[relicKey] = next;
		values = { ...values };
	}

	function apply(): void {
		const committed = { ...values };
		// Ranks follow the staged counts exactly the way PalSavTools' ability
		// write does: setting a type's count invests that many effigies.
		Object.assign(player.status_point_list, deriveStatusPatches(committed, relics));
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
			<p
				class="text-surface-400 flex items-center gap-1.5 text-[10px] font-semibold tracking-widest uppercase"
			>
				<Gem size={12} class="text-secondary-500" />
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
						<Grid3x3 size={13} />
					</button>
					<button
						type="button"
						class="rounded-sm p-1 transition-colors {viewMode === 'list'
							? 'bg-surface-200 dark:bg-surface-700 text-on-surface'
							: 'text-surface-400 hover:text-on-surface'}"
						onclick={() => (viewMode = 'list')}
						aria-label={m.effigies_list_view()}
					>
						<List size={13} />
					</button>
				</div>
				<div class="bg-surface-200 dark:bg-surface-700 h-4 w-px"></div>
				<Button variant="ghost" onclick={setAllToMax} disabled={!supported} class="!text-xs">
					<Zap size={13} class="mr-1" />{m.max_all_abilities()}
				</Button>
			</div>
		</div>

		{#if !supported}
			<p class="text-warning-500 flex items-center gap-1.5 text-xs">
				<TriangleAlert size={13} />
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
					{@const pct = cap > 0 ? (val / cap) * 100 : 0}
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
						<span class="text-surface-400 w-5 shrink-0 font-mono text-[9px] tabular-nums">
							#{relicIndexLabel(relicKey)}
						</span>
						<div class="min-w-0 flex-1">
							<p class="text-on-surface truncate text-xs leading-tight">
								{entry.localized_name}
								{#if illegal}<TriangleAlert size={10} class="text-warning-500 ml-1 inline" />{/if}
							</p>
							<p class="text-surface-400 text-[9px] leading-tight">
								{m.rank_hint({ rank: storedRank(relicKey), max: entry.max_rank })}
							</p>
						</div>
						<button
							type="button"
							class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 text-surface-400 hover:bg-surface-200 dark:hover:bg-surface-800 flex h-5 w-5 shrink-0 items-center justify-center rounded-sm border text-xs leading-none transition-colors disabled:opacity-30"
							onclick={() => bump(relicKey, -1)}
							disabled={!supported || val <= 0}
						>
							−
						</button>
						<!-- Clickable progress bar = the value editor -->
						<div
							class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 relative h-6 flex-1 cursor-pointer overflow-hidden rounded-sm border"
							role="slider"
							tabindex="0"
							aria-valuemin="0"
							aria-valuemax={cap}
							aria-valuenow={val}
							onkeydown={(e) => barKeydown(relicKey, e)}
							onclick={(e) => barClick(relicKey, e)}
						>
							<div
								class="h-full rounded-sm transition-all duration-200 {val >= cap
									? 'bg-success-500'
									: 'bg-secondary-500'}"
								style="width: {pct}%"
							></div>
							<span
								class="pointer-events-none absolute inset-0 flex items-center justify-center text-[10px] font-semibold text-white tabular-nums"
							>
								{val}
							</span>
						</div>
						<button
							type="button"
							class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 text-surface-400 hover:bg-surface-200 dark:hover:bg-surface-800 flex h-5 w-5 shrink-0 items-center justify-center rounded-sm border text-xs leading-none transition-colors disabled:opacity-30"
							onclick={() => bump(relicKey, 1)}
							disabled={!supported || val >= cap}
						>
							+
						</button>
						<span class="text-surface-400 w-8 shrink-0 text-right text-[9px] tabular-nums"
							>/{cap}</span
						>
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
					{@const pct = cap > 0 ? (val / cap) * 100 : 0}
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
									<span class="text-surface-400 font-mono text-[9px] tabular-nums">
										#{relicIndexLabel(relicKey)}
									</span>
									<p class="text-on-surface truncate text-xs font-medium">
										{entry.localized_name}
										{#if illegal}<TriangleAlert
												size={10}
												class="text-warning-500 ml-1 inline"
											/>{/if}
									</p>
								</div>
								<p class="text-surface-400 mt-0.5 text-[9px] leading-tight">
									{m.rank_hint({ rank: storedRank(relicKey), max: entry.max_rank })}
								</p>
							</div>
						</div>
						<!-- Controls: − | clickable progress bar | +  /max -->
						<div class="flex items-center gap-1.5">
							<button
								type="button"
								class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 text-surface-400 hover:bg-surface-200 dark:hover:bg-surface-800 flex h-6 w-6 shrink-0 items-center justify-center rounded-sm border text-sm leading-none transition-colors disabled:opacity-30"
								onclick={() => bump(relicKey, -1)}
								disabled={!supported || val <= 0}
							>
								−
							</button>
							<div
								class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 relative h-7 flex-1 cursor-pointer overflow-hidden rounded-sm border"
								role="slider"
								tabindex="0"
								aria-valuemin="0"
								aria-valuemax={cap}
								aria-valuenow={val}
								onkeydown={(e) => barKeydown(relicKey, e)}
								onclick={(e) => barClick(relicKey, e)}
							>
								<div
									class="h-full rounded-sm transition-all duration-200 {val >= cap
										? 'bg-success-500'
										: 'bg-secondary-500'}"
									style="width: {pct}%"
								></div>
								<span
									class="pointer-events-none absolute inset-0 flex items-center justify-center text-[11px] font-semibold text-white tabular-nums"
								>
									{val}
								</span>
							</div>
							<button
								type="button"
								class="border-surface-200 dark:border-surface-700 bg-surface-100 dark:bg-surface-900 text-surface-400 hover:bg-surface-200 dark:hover:bg-surface-800 flex h-6 w-6 shrink-0 items-center justify-center rounded-sm border text-sm leading-none transition-colors disabled:opacity-30"
								onclick={() => bump(relicKey, 1)}
								disabled={!supported || val >= cap}
							>
								+
							</button>
							<span class="text-surface-400 w-8 shrink-0 text-right text-[10px] tabular-nums"
								>/{cap}</span
							>
						</div>
					</div>
				{/each}
			</div>
		{/if}

		{#if unknownTypes.length > 0}
			<p class="text-warning-500 flex items-start gap-1.5 text-xs">
				<TriangleAlert size={13} class="mt-0.5 shrink-0" />
				{m.effigies_unknown_types({ types: unknownTypes.join(', ') })}
			</p>
		{/if}

		<div class="border-surface-200 dark:border-surface-800 flex items-center gap-2 border-t pt-2">
			<Button variant="primary" onclick={apply} disabled={!supported || !dirty}>
				<Check size={14} class="mr-1" />{m.apply_effigies()}
			</Button>
			<Button variant="ghost" onclick={reset} disabled={!dirty}>
				<RotateCcw size={13} class="mr-1" />{m.reset()}
			</Button>
		</div>
	</div>
{/if}
