<script lang="ts">
	import type { Boss } from '$types';
	import { palsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { bossPalKey, humanizeSpawnerId } from '../utils';
	import Popup from './Popup.svelte';
	import Badge from './Badge.svelte';
	import * as m from '$i18n/messages';

	let {
		boss
	}: {
		boss: Boss & { defeated?: boolean; localized_name?: string };
	} = $props();

	const coords = $derived({ x: boss.x, y: boss.y, z: boss.z });
	const palKey = $derived(bossPalKey(boss.character_id));
	const palData = $derived(palKey ? palsData.getByKey(palKey) : undefined);
	// Human bosses carry character_id "None"; their spawner_id is the only identifier.
	const title = $derived(
		boss.localized_name || palData?.localized_name || humanizeSpawnerId(boss.spawner_id)
	);
</script>

<Popup {title} subtitle={humanizeSpawnerId(boss.spawner_id)} {coords}>
	{#snippet icon()}
		{#if palKey}
			<img src={assetLoader.loadMenuImage(palKey)} alt="" class="h-8 w-8 rounded-full" />
		{/if}
	{/snippet}
	{#snippet action()}
		<Badge>{m.level_abbr_value({ value: boss.level })}</Badge>
	{/snippet}
	{#snippet content()}
		<Badge variant={boss.defeated ? 'success' : 'error'}>
			{boss.defeated ? m.defeated() : m.not_defeated()}
		</Badge>
		{#if palData?.description}
			<p class="text-xs">{palData.description}</p>
		{/if}
	{/snippet}
</Popup>
