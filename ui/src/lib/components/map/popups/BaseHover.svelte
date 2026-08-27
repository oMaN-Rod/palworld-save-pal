<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Base } from '$types';
	import Hover from './Hover.svelte';
	import InfoRow from './InfoRow.svelte';
	import * as m from '$i18n/messages';

	let {
		base,
		guildName
	}: {
		base: Base;
		guildName?: string;
	} = $props();
</script>

<Hover title={base.name ?? base.id} subtitle={base.id} coords={base.location}>
	{#snippet icon()}
		<Icon icon="tabler:home" class="text-primary-500 h-4 w-4" />
	{/snippet}
	{#snippet content()}
		{#if guildName}
			<InfoRow icon={'tabler:users'} label={m.guild({ count: 1 })} value={guildName} />
		{/if}
		<InfoRow icon={'tabler:fence'} label="Area" value={String(base.area_range)} />
	{/snippet}
</Hover>
