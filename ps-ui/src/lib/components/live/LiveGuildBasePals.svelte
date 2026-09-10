<script lang="ts">
	import { Spinner } from '$components/ui';
	import { PalGrid } from '$components/pal';
	import LivePalBadge from './LivePalBadge.svelte';
	import LivePalboxPager from './LivePalboxPager.svelte';
	import { seatPals } from './liveGuild.utils';
	import type {
		GameBasePalsJson,
		GameGuildBaseJson,
		GamePalJson,
		GameRefusalJson
	} from '$states/gameState.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		bases = [],
		basePage = 0,
		onBasePageChange,
		basePals = null,
		basePalsError = null,
		basePalsLoading = false,
		busy = false,
		onEditPal,
		onAddPal,
		editDisabledReason,
		addDisabledReason
	}: {
		bases?: GameGuildBaseJson[];
		basePage?: number;
		onBasePageChange?: (page: number) => void;
		basePals?: GameBasePalsJson | null;
		basePalsError?: GameRefusalJson | null;
		basePalsLoading?: boolean;
		busy?: boolean;
		onEditPal?: (pal: GamePalJson, slotIndex: number) => void;
		onAddPal?: (slotIndex: number) => void;
		editDisabledReason?: string;
		addDisabledReason?: string;
	} = $props();

	const currentBase = $derived(bases[basePage] ?? null);
	const matches = $derived(!!currentBase && basePals?.baseId === currentBase.id);
	const pals = $derived(matches ? (basePals?.pals ?? []) : []);
	const slotCount = $derived(
		(matches ? basePals?.slotNum : null) ?? currentBase?.palSlotNum ?? 0
	);
	const seats = $derived(seatPals(pals, slotCount));
</script>

<div class="flex flex-col gap-2">
	{#if bases.length > 1}
		<LivePalboxPager
			id="live-base-pager"
			page={basePage}
			pageCount={bases.length}
			disabled={basePalsLoading}
			labelFor={(index) => bases[index]?.name ?? String(index + 1)}
			onPageChange={(page) => onBasePageChange?.(page)}
		/>
	{/if}

	{#if currentBase}
		<div class="flex flex-wrap items-baseline gap-x-3">
			<span class="text-base font-medium">{currentBase.name ?? '—'}</span>
			<span class="text-surface-400 text-xs">
				{m.live_guild_base_camp_level()} {currentBase.level ?? '—'}
			</span>
		</div>
	{/if}

	{#if basePalsError}
		<p class="text-error-400 text-sm">{basePalsError.error}</p>
	{:else if basePalsLoading && seats.length === 0}
		<div class="flex items-center gap-3 py-4">
			<Spinner size="size-8" />
			<span class="text-surface-300 text-sm">{m.loading_entity({ entity: c.pals })}</span>
		</div>
	{:else if seats.length === 0}
		<p class="text-surface-400 text-sm">{m.no_entity_yet({ entity: c.pals })}</p>
	{:else}
		<PalGrid id="live-base-pals" data-testid="live-base-pals">
			{#each seats as seat (seat.position)}
				<LivePalBadge
					pal={seat.pal}
					slotIndex={seat.position}
					healBusy={busy}
					onEdit={onEditPal}
					onAdd={onAddPal}
					palEditDisabledReason={editDisabledReason}
					addDisabledReason={addDisabledReason}
					healDisabledReason={m.live_guild_base_pal_ops_unavailable()}
				/>
			{/each}
		</PalGrid>
	{/if}
</div>
