<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Player } from '$types';
	import { Button } from '$components/ui';
	import { getAppState, getNavigationState } from '$states';
	import Popup from './Popup.svelte';
	import Badge from './Badge.svelte';
	import InfoRow from './InfoRow.svelte';
	import * as m from '$i18n/messages';

	let {
		player
	}: {
		player: Player;
	} = $props();

	const appState = getAppState();
	const nav = getNavigationState();

	const guildName = $derived.by(() => {
		if (!player.guild_id) return null;
		const guild = appState.guilds?.[player.guild_id];
		if (guild) return guild.name;
		return appState.guildSummaries?.[player.guild_id]?.name || null;
	});

	const palCount = $derived(player.pals ? Object.keys(player.pals).length : 0);
	const dpsCount = $derived(player.dps ? Object.keys(player.dps).length : 0);

	function handleEdit(event: MouseEvent) {
		event.stopPropagation();
		event.preventDefault();
		if (appState.selectedPlayerUid !== player.uid) {
			appState.selectedPlayer = player;
			appState.selectedPlayerUid = player.uid;
		}
		nav.saveAndNavigate('/edit');
	}
</script>

<Popup title={player.nickname} coords={player.location}>
	{#snippet action()}
		<Badge>{m.level_abbr_value({ value: player.level })}</Badge>
	{/snippet}
	{#snippet content()}
		{#if guildName}
			<InfoRow icon={'tabler:users'} label={m.guild({ count: 1 })} value={guildName} />
		{/if}
		<InfoRow icon={'tabler:heart'} iconClass="text-error-500" label={m.hp()} value={String(player.hp)} />
		<InfoRow icon={'tabler:device-gamepad-2'} label={m.pal({ count: 2 })} value={String(palCount)} />
		{#if dpsCount > 0}
			<InfoRow icon={'tabler:swords'} iconClass="text-warning-500" label="DPS" value={String(dpsCount)} />
		{/if}
		<InfoRow
			icon={'tabler:clock'}
			label={m.last_online()}
			value={new Date(player.last_online_time).toLocaleString()}
		/>
	{/snippet}
	{#snippet actions()}
		<Button variant="secondary" onclick={handleEdit}>
			<Icon icon="tabler:pencil" class="h-3.5 w-3.5" />
			{m.edit()}
		</Button>
	{/snippet}
</Popup>
