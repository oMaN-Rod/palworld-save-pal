<script lang="ts">
	import { Card } from '$components/ui';
	import LivePartyCard from './LivePartyCard.svelte';
	import type { GamePalJson } from '$states/gameState.svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const PARTY_SIZE = 5;

	let {
		party = [],
		partyReadable = false,
		levelCap,
		healBusy = false,
		healingPalId = null,
		healDisabledReason,
		editDisabledReason,
		addDisabledReason,
		onHeal,
		onEdit,
		onAdd
	}: {
		party?: GamePalJson[];
		partyReadable?: boolean;
		levelCap?: number;
		healBusy?: boolean;
		healingPalId?: string | null;
		healDisabledReason?: string;
		editDisabledReason?: string;
		addDisabledReason?: string;
		onHeal?: (pal: GamePalJson) => void;
		onEdit?: (pal: GamePalJson) => void;
		onAdd?: (slotIndex: number) => void;
	} = $props();

	const slots = $derived.by(() => {
		const byIndex = new Map<number, GamePalJson>();
		for (const pal of party) {
			if (!byIndex.has(pal.slotIndex)) byIndex.set(pal.slotIndex, pal);
		}
		const size = Math.max(PARTY_SIZE, ...[...byIndex.keys()].map((index) => index + 1));
		return Array.from({ length: size }, (_, index) => ({ index, pal: byIndex.get(index) }));
	});
</script>

<Card rounded="rounded-sm">
	<h4 class="h4 mb-2">{m.party()}</h4>
	{#if !partyReadable}
		<p class="text-surface-400 text-sm">{m.live_party_unavailable()}</p>
	{:else if party.length === 0 && !onAdd}
		<p class="text-surface-400 text-sm">{m.no_entity_yet({ entity: c.pals })}</p>
	{:else}
		<div id="live-party" class="flex flex-col space-y-2">
			{#each slots as slot (slot.index)}
				<LivePartyCard
					pal={slot.pal}
					slotIndex={slot.index}
					{levelCap}
					{healBusy}
					healing={slot.pal !== undefined && healingPalId === slot.pal.instanceId}
					{healDisabledReason}
					{editDisabledReason}
					{addDisabledReason}
					{onHeal}
					{onEdit}
					{onAdd}
				/>
			{/each}
		</div>
	{/if}
</Card>
