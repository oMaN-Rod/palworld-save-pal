<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import X from '@lucide/svelte/icons/x';
	import type { RelicPoint } from '$types';
	import { relicTypeIcon } from '../styles';
	import Hover from './Hover.svelte';
	import Badge from './Badge.svelte';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: RelicPoint;
	} = $props();

	const coords = $derived({ x: point.x, y: point.y, z: point.z });
</script>

<Hover title={point.localized_name} {coords}>
	{#snippet icon()}
		<img src={relicTypeIcon(point.relic_type)} alt="" class="h-4 w-4" />
	{/snippet}
	{#snippet content()}
		{#if point.unlocked !== undefined}
			<Badge variant={point.unlocked ? 'success' : 'error'}>
				{#if point.unlocked}
					<Check class="h-3 w-3 shrink-0" />
					{m.collected()}
				{:else}
					<X class="h-3 w-3 shrink-0" />
					{m.not_collected()}
				{/if}
			</Badge>
		{/if}
	{/snippet}
</Hover>
