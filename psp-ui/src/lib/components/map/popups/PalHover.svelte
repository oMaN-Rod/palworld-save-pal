<script lang="ts">
	import type { MapObject } from '$types';
	import { palsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import Hover from './Hover.svelte';
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
</script>

<Hover title={palData?.localized_name ?? point.pal ?? ''} subtitle={point.pal} {coords}>
	{#snippet icon()}
		{#if point.pal}
			<img src={assetLoader.loadMenuImage(point.pal)} alt="" class="h-6 w-6 rounded-full" />
		{/if}
	{/snippet}
	{#snippet action()}
		<Badge variant={isPredator ? 'error' : 'warning'}>
			{isPredator ? m.predator() : m.alpha()}
		</Badge>
	{/snippet}
</Hover>
