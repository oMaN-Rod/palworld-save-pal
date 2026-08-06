<script lang="ts">
	// OwnerSelect — searchable player dropdown for Save Mode. Shared by the
	// list-mode page and the graph-mode side panel so both behave identically.
	//
	// The listbox is portaled to <body> and fixed-positioned via Floating UI
	// (same rationale as PalPicker): a plain `absolute` listbox trapped inside
	// the .card / side-panel stacking contexts gets painted over and clipped.
	import { computePosition, flip, shift, offset, autoUpdate } from '@floating-ui/dom';
	import { portal } from '$utils';
	import * as m from '$i18n/messages';
	import type { PlayerSummaryT } from '$lib/breeding/types';

	let {
		players = [],
		ownerUid = null,
		onownerUidChange
	}: {
		players?: PlayerSummaryT[];
		ownerUid?: string | null;
		onownerUidChange?: (uid: string | null) => void;
	} = $props();

	let ownerSearch = $state('');
	let ownerFocus = $state(false);
	let ownerBlurTimer: ReturnType<typeof setTimeout> | undefined;
	let inputEl: HTMLInputElement = $state(null!);
	let floatingEl: HTMLDivElement = $state(null!);
	let cleanup: (() => void) | null = null;

	const filteredPlayers = $derived(
		ownerSearch
			? players.filter((pl) => {
					const q = ownerSearch.toLowerCase();
					return (
						pl.name.toLowerCase().includes(q) ||
						pl.uid.toLowerCase().includes(q) ||
						(pl.guild_name ?? '').toLowerCase().includes(q)
					);
				})
			: players
	);

	function select(uid: string | null) {
		ownerUid = uid;
		ownerSearch = '';
		ownerFocus = false;
		onownerUidChange?.(uid);
	}

	$effect(() => {
		if (!ownerFocus || !inputEl || !floatingEl) return;
		cleanup?.();
		const update = () => {
			if (!inputEl || !floatingEl) return;
			computePosition(inputEl, floatingEl, {
				placement: 'bottom-start',
				strategy: 'fixed',
				middleware: [offset(4), flip(), shift({ padding: 6 })]
			}).then(({ x, y }) => {
				Object.assign(floatingEl.style, {
					left: `${x}px`,
					top: `${y}px`,
					width: `${inputEl.offsetWidth}px`
				});
			});
		};
		update();
		cleanup = autoUpdate(inputEl, floatingEl, update);
		return () => {
			cleanup?.();
			cleanup = null;
		};
	});
</script>

<div>
	<span class="block text-xs font-semibold text-surface-400 uppercase tracking-wider mb-1">
		{m.breeding_owner()}
	</span>
	<div class="relative">
		<input
			type="text"
			bind:this={inputEl}
			class="input text-xs"
			value={ownerUid ? players.find((p) => p.uid === ownerUid)?.name ?? ownerUid : ownerSearch}
			placeholder={m.breeding_owner_search()}
			oninput={(e) => {
				ownerSearch = (e.currentTarget as HTMLInputElement).value;
				if (ownerUid) select(null);
			}}
			onfocus={() => {
				if (ownerBlurTimer) clearTimeout(ownerBlurTimer);
				ownerFocus = true;
			}}
			onblur={() => {
				ownerBlurTimer = setTimeout(() => {
					ownerFocus = false;
				}, 200);
			}}
			role="combobox"
			aria-expanded={ownerFocus}
			aria-haspopup="listbox"
			aria-controls="owner-listbox"
		/>
		{#if ownerFocus}
			<div
				bind:this={floatingEl}
				{@attach portal()}
				class="bg-surface-950 border border-surface-700/40 rounded-md shadow-xl flex flex-col max-h-48 overflow-y-auto min-w-52"
				style="position: fixed; z-index: 99999;"
				role="listbox"
				id="owner-listbox"
			>
<button
						class="w-full text-left px-3 py-1.5 text-xs text-surface-300 hover:bg-surface-800 transition-colors border-b border-surface-700/20 last:border-b-0 {ownerUid === null ? 'bg-primary-500/10' : ''}"
						onmousedown={() => select(null)}
					>
						{m.breeding_all_players()}
					</button>
					{#if players.length === 0}
						<div class="px-3 py-2 text-xs text-surface-400">{m.breeding_no_players()}</div>
					{:else}
						{#each filteredPlayers as pl (pl.uid)}
					<button
						class="w-full text-left px-3 py-1.5 text-xs text-surface-50 hover:bg-surface-800 transition-colors border-b border-surface-700/20 last:border-b-0 {ownerUid === pl.uid ? 'bg-primary-500/10' : ''}"
						onmousedown={() => select(pl.uid)}
					>
						{pl.name}
						<span class="text-surface-400 ml-1"
							>({pl.pal_count} pals{pl.guild_name ? `, ${pl.guild_name}` : ''})</span
						>
					</button>
				{/each}
					{#if filteredPlayers.length === 0 && ownerSearch}
						<div class="px-3 py-2 text-xs text-surface-400">{m.breeding_owner_no_match()}</div>
					{/if}
				{/if}
			</div>
		{/if}
	</div>
</div>