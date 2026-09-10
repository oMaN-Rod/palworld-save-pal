<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Base } from '$types';
	import { Button } from '$components/ui';
	import { itemsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { ASSET_DATA_PATH } from '$types/icons';
	import Popup from './Popup.svelte';
	import InfoRow from './InfoRow.svelte';
	import * as m from '$i18n/messages';

	let {
		base,
		guildName,
		onExport,
		onDeleteBase
	}: {
		base: Base;
		guildName?: string;
		onExport?: (base: Base) => void;
		onDeleteBase?: (base: Base) => void;
	} = $props();

	const palCount = $derived(base.pals ? Object.keys(base.pals).length : 0);
	const containerCount = $derived(Object.keys(base.storage_containers || {}).length);
	const baseValue = $derived.by(() => {
		if (!base.storage_containers) return '0';
		const slots = Object.values(base.storage_containers).flatMap((container) => container.slots);
		const total = slots.reduce((sum, slot) => {
			const itemData = itemsData.getByKey(slot.static_id);
			return sum + (itemData ? itemData.details.price * slot.count : 0);
		}, 0);
		return total.toLocaleString();
	});

	const goldCoinIcon = $derived.by(() => {
		const goldCoinData = itemsData.getByKey('money');
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${goldCoinData?.details.icon}.webp`);
	});
</script>

<Popup title={base.name ?? base.id} subtitle={base.id} coords={base.location}>
	{#snippet icon()}
		<Icon icon="tabler:home" class="text-primary-500 h-5 w-5" />
	{/snippet}
	{#snippet content()}
		{#if guildName}
			<InfoRow icon={'tabler:users'} label={m.guild({ count: 1 })} value={guildName} />
		{/if}
		<InfoRow icon={'tabler:fence'} label="Area" value={String(base.area_range)} />
		<InfoRow icon={'ph:paw-print'} label={m.pal({ count: 2 })} value={String(palCount)} />
		<InfoRow icon={'tabler:package'} label={m.storage()}>
			<div class="flex items-center gap-2">
				<span class="font-mono text-xs">{containerCount}</span>
				<span class="flex items-center gap-0.5">
					<img src={goldCoinIcon} alt="" class="h-4 w-4" />
					<span class="font-mono text-xs">{baseValue}</span>
				</span>
			</div>
		</InfoRow>
	{/snippet}
	{#snippet actions()}
		{#if onExport}
			<Button variant="secondary" onclick={() => onExport?.(base)}>Export blueprint</Button>
		{/if}
		{#if onDeleteBase}
			<Button variant="danger" onclick={() => onDeleteBase?.(base)}>Delete base</Button>
		{/if}
	{/snippet}
</Popup>
