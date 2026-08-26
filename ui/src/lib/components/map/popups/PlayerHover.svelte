<script lang="ts">
	import Clock from '@lucide/svelte/icons/clock';
	import Heart from '@lucide/svelte/icons/heart';
	import type { Player } from '$types';
	import Hover from './Hover.svelte';
	import Badge from './Badge.svelte';
	import InfoRow from './InfoRow.svelte';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: Player;
	} = $props();
</script>

<Hover title={point.nickname} coords={point.location}>
	{#snippet action()}
		<Badge>{m.level_abbr_value({ value: point.level })}</Badge>
	{/snippet}
	{#snippet content()}
		<InfoRow icon={Heart} iconClass="text-error-500" label={m.hp()} value={String(point.hp)} />
		<InfoRow
			icon={Clock}
			label={m.last_online()}
			value={new Date(point.last_online_time).toLocaleString()}
		/>
	{/snippet}
</Hover>
