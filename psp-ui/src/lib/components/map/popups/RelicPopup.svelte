<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { RelicPoint } from '$types';
	import { relicTypeIcon } from '../style/styles';
	import Popup from './Popup.svelte';
	import Badge from './Badge.svelte';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: RelicPoint;
	} = $props();

	const coords = $derived({ x: point.x, y: point.y, z: point.z });
</script>

<Popup title={point.localized_name} {coords}>
	{#snippet icon()}
		<img src={relicTypeIcon(point.relic_type)} alt="" class="h-5 w-5" />
	{/snippet}
	{#snippet content()}
		{#if point.unlocked !== undefined}
			<Badge variant={point.unlocked ? 'success' : 'error'}>
				{#if point.unlocked}
					<Icon icon="tabler:check" class="h-3 w-3 shrink-0" />
					{m.collected()}
				{:else}
					<Icon icon="tabler:x" class="h-3 w-3 shrink-0" />
					{m.not_collected()}
				{/if}
			</Badge>
		{/if}
	{/snippet}
</Popup>
