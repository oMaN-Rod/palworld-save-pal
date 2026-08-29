<script lang="ts">
	import type { MapObject, Pal } from '$types';
	import { palsData } from '$lib/data';
	import { PalBadge } from '$components/pal';
	import Popup from './Popup.svelte';
	import Badge from './Badge.svelte';
	import * as m from '$i18n/messages';

	let {
		point,
		isPredator = false
	}: {
		point: MapObject;
		isPredator?: boolean;
	} = $props();

	const coords = $derived({ x: point.x, y: point.y, z: point.z });
	const palData = $derived(palsData.getByKey(point.pal ?? ''));

	const pal = $derived({
		instance_id: '',
		character_key: point.pal,
		character_id: point.pal,
		is_sick: false,
		is_predator: isPredator,
		is_boss: !isPredator
	} as Pal);
</script>

<Popup title={palData?.localized_name ?? point.pal ?? ''} subtitle={point.pal} {coords}>
	{#snippet icon()}
		<PalBadge
			{pal}
			onMove={() => {}}
			onAdd={() => {}}
			onClone={() => {}}
			onDelete={() => {}}
			disabled
		/>
	{/snippet}
	{#snippet action()}
		<Badge variant={isPredator ? 'error' : 'warning'}>
			{isPredator ? m.predator() : m.alpha()}
		</Badge>
	{/snippet}
	{#snippet content()}
		{#if palData?.description}
			<p class="text-xs">{palData.description}</p>
		{/if}
	{/snippet}
</Popup>
