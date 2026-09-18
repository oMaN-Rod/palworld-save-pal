<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Server } from '$types';
	import { Card } from '$components/ui';
	import { getServerState } from '$states';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';

	let {
		server,
		selected = false,
		onselect,
		onstart,
		onstop
	} = $props<{
		server: Server;
		selected?: boolean;
		onselect: (server: Server) => void;
		onstart: (server: Server) => void;
		onstop: (server: Server) => void;
	}>();

	const serverState = getServerState();

	const isRunning = $derived(server.status?.running ?? false);
	const starting = $derived(!isRunning && (serverState.starting[server.id] ?? false));
	const isDocker = $derived(server.server_type === 'docker');

	const statusColor = $derived.by(() => {
		const status = server.status?.status ?? 'not_found';
		switch (status) {
			case 'running':
				return 'bg-green-500';
			case 'exited':
				return 'bg-red-500';
			case 'created':
				return 'bg-yellow-500';
			case 'paused':
				return 'bg-orange-500';
			default:
				return 'bg-gray-500';
		}
	});

	const statusText = $derived(server.status?.status ?? 'unknown');
</script>

<div
	class={cn(
		'group relative w-full text-left transition-all',
		selected ? 'ring-secondary-500 rounded-sm ring-2' : ''
	)}
>
	<button
		class="focus-visible:outline-primary-300 absolute inset-0 z-0 cursor-pointer rounded-sm focus-visible:outline-2 focus-visible:outline-offset-2"
		aria-label={server.name}
		aria-pressed={selected}
		onclick={() => onselect(server)}
	></button>
	<Card class="group-hover:bg-surface-800 pointer-events-none">
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<div class={cn('h-3 w-3 rounded-full', statusColor)}></div>
				<div>
					<h4 class="font-bold">{server.name}</h4>
					{#if isDocker}
						<p class="text-surface-400 text-xs">{server.container_name}</p>
					{/if}
				</div>
			</div>
			<div class="flex items-center gap-3">
				{#if isRunning}
					<span class="text-surface-400 flex items-center gap-1 text-sm">
						<Icon icon="tabler:users" size={14} />
						{server.player_count ?? 0}
					</span>
				{/if}
				<button
					class={cn(
						'btn btn-sm pointer-events-auto rounded-sm p-1.5',
						isRunning
							? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
							: 'bg-green-500/20 text-green-400 hover:bg-green-500/30'
					)}
					data-server-toggle
					aria-label={isRunning ? m.servers_stop() : m.servers_start()}
					disabled={starting}
					aria-busy={starting}
					onclick={() => {
						if (starting) return;
						isRunning ? onstop(server) : onstart(server);
					}}
				>
					{#if isRunning}
						<Icon icon="tabler:square" size={14} />
					{:else if starting}
						<Icon icon="tabler:loader-2" size={14} class="animate-spin" />
					{:else}
						<Icon icon="tabler:player-play" size={14} />
					{/if}
				</button>
			</div>
		</div>
		<div class="text-surface-400 mt-2 flex gap-4 text-xs">
			<span
				class={cn(
					'rounded-sm px-1.5 py-0.5 text-[10px] font-medium uppercase',
					server.server_type === 'native'
						? 'bg-blue-500/15 text-blue-400'
						: 'bg-cyan-500/15 text-cyan-400'
				)}
			>
				{server.server_type === 'native' ? 'Native' : 'Docker'}
			</span>
			<span>Port: {server.game_port}</span>
			<span>Players: {server.max_players}</span>
			<span class="capitalize">{statusText}</span>
		</div>
	</Card>
</div>
