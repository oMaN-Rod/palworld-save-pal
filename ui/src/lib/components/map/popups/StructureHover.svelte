<script lang="ts">
	import type { BaseStructure } from '$types';
	import { baseStructuresData, buildingsData } from '$lib/data';
	import { getAppState } from '$states';
	import { structureInfo } from '../structureInfo';
	import { structureColors } from '../mapColors.svelte';
	import Hover from './Hover.svelte';
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

<Hover title={info.name} subtitle={structure.map_object_id} {coords}>
	{#snippet icon()}
		<span class="block h-3 w-3 rounded-full" style="background-color: {swatchColor}"></span>
	{/snippet}
	{#snippet content()}
		<InfoRow icon={'tabler:box'} label={m.type({ count: 1 })} value={info.typeA} />
		<InfoRow
			icon={'tabler:heart'}
			iconClass="text-error-500"
			label={m.hp()}
			value="{info.hp} / {info.hpMax}"
		/>
	{/snippet}
</Hover>
