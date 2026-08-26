<script lang="ts">
	import Award from '@lucide/svelte/icons/award';
	import Box from '@lucide/svelte/icons/box';
	import Heart from '@lucide/svelte/icons/heart';
	import Ruler from '@lucide/svelte/icons/ruler';
	import User from '@lucide/svelte/icons/user';
	import type { BaseStructure } from '$types';
	import { baseStructuresData, buildingsData } from '$lib/data';
	import { getAppState } from '$states';
	import { structureInfo } from '../structureInfo';
	import { structureColors } from '../mapColors.svelte';
	import Popup from './Popup.svelte';
	import InfoRow from './InfoRow.svelte';
	import * as m from '$i18n/messages';

	let { structure }: { structure: BaseStructure } = $props();

	const appState = getAppState();
	const info = $derived(
		structureInfo(
			structure,
			baseStructuresData.footprints,
			buildingsData.buildings,
			appState.playerSummaries
		)
	);
	const swatchColor = $derived.by(() => {
		const colors = structureColors();
		return colors[info.typeA] ?? colors.Other;
	});
	const coords = $derived({ x: structure.x, y: structure.y, z: structure.z });
</script>

<Popup title={info.name} subtitle={structure.map_object_id} {coords}>
	{#snippet icon()}
		<span class="block h-3 w-3 rounded-full" style="background-color: {swatchColor}"></span>
	{/snippet}
	{#snippet content()}
		{#if info.description}
			<p class="text-xs">{info.description}</p>
		{/if}
		<InfoRow icon={Box} label={m.type({ count: 1 })} value={info.typeA} />
		<InfoRow
			icon={Heart}
			iconClass="text-error-500"
			label={m.hp()}
			value="{info.hp} / {info.hpMax}"
		/>
		<InfoRow
			icon={Ruler}
			label={m.size()}
			value="{info.sizeM.x.toFixed(2)} x {info.sizeM.y.toFixed(2)} x {info.sizeM.z.toFixed(2)} m"
		/>
		{#if info.rank !== undefined}
			<InfoRow icon={Award} label="Rank" value={String(info.rank)} />
		{/if}
		{#if info.builder}
			<InfoRow icon={User} label="Builder" value={info.builder} />
		{/if}
	{/snippet}
</Popup>
