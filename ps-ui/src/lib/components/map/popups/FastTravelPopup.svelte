<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { MapUnlockPoint } from '$types';
	import Popup from './Popup.svelte';
	import Badge from './Badge.svelte';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: MapUnlockPoint;
	} = $props();

	const coords = $derived({ x: point.x, y: point.y, z: point.z });
</script>

<Popup title={point.localized_name} subtitle={m.fast_travel()} {coords}>
	{#snippet icon()}
		<Icon icon="tabler:navigation" class="text-primary-500 h-5 w-5" />
	{/snippet}
	{#snippet content()}
		{#if point.unlocked !== undefined}
			<Badge variant={point.unlocked ? 'success' : 'error'}>
				{#if point.unlocked}
					<Icon icon="tabler:lock-open" class="h-3 w-3 shrink-0" />
					{m.unlocked()}
				{:else}
					<Icon icon="tabler:lock" class="h-3 w-3 shrink-0" />
					{m.locked()}
				{/if}
			</Badge>
		{/if}
	{/snippet}
</Popup>
