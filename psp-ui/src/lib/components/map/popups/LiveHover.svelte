<script lang="ts">
	import type { LiveActorJson } from '$lib/signal/session.svelte';
	import { getLiveActors } from '$lib/data/liveActors.svelte';
	import { Progress } from '$components/ui';
	import Hover from './Hover.svelte';
	import Badge from './Badge.svelte';
	import InfoRow from './InfoRow.svelte';
	import { liveKindLabel } from './labels';
	import * as m from '$i18n/messages';

	let { actor }: { actor: LiveActorJson } = $props();

	const coords = $derived({ x: actor.x, y: actor.y, z: actor.z });
	const ownerName = $derived.by(() => {
		if (!actor.owner) return undefined;
		return getLiveActors().actors.find((a) => a.id === actor.owner)?.name;
	});
</script>

<Hover
	title={actor.name || liveKindLabel(actor.kind)}
	subtitle={actor.name ? liveKindLabel(actor.kind) : undefined}
	{coords}
>
	{#snippet action()}
		{#if actor.level !== undefined}
			<Badge>{m.level_abbr_value({ value: actor.level })}</Badge>
		{/if}
	{/snippet}
	{#snippet content()}
		{#if actor.hp !== undefined && actor.maxHp !== undefined}
			<InfoRow icon={'tabler:heart'} iconClass="text-error-500" label={m.hp()}>
				<Progress value={actor.hp} max={actor.maxHp} height="h-4" color="green" />
			</InfoRow>
		{/if}
		{#if actor.guild}
			<InfoRow icon={'tabler:users'} label={m.guild({ count: 1 })} value={actor.guild} />
		{/if}
		{#if ownerName}
			<InfoRow icon={'ph:paw-print'} label={m.live_owner()} value={ownerName} />
		{/if}
	{/snippet}
</Hover>
