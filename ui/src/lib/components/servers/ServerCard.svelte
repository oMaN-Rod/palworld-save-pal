<script lang="ts">
	import type { Server } from '$types';
	import { Card } from '$components/ui';
	import { Users, Play, Square } from 'lucide-svelte';
	import { cn } from '$theme';

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

	const isRunning = $derived(server.status?.running ?? false);

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

<button
	class={cn(
		'w-full text-left transition-all',
		selected ? 'ring-2 ring-secondary-500 rounded-sm' : ''
	)}
	onclick={() => onselect(server)}
>
	<Card class="hover:bg-surface-800 cursor-pointer">
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<div class={cn('h-3 w-3 rounded-full', statusColor)}></div>
				<div>
					<h4 class="font-bold">{server.name}</h4>
					<p class="text-surface-400 text-xs">{server.container_name}</p>
				</div>
			</div>
			<div class="flex items-center gap-3">
				{#if isRunning}
					<span class="text-surface-400 flex items-center gap-1 text-sm">
						<Users size={14} />
						{server.player_count ?? 0}
					</span>
				{/if}
				<button
					class={cn(
						'btn btn-sm rounded-sm p-1.5',
						isRunning
							? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
							: 'bg-green-500/20 text-green-400 hover:bg-green-500/30'
					)}
					onclick={(e) => {
						e.stopPropagation();
						isRunning ? onstop(server) : onstart(server);
					}}
				>
					{#if isRunning}
						<Square size={14} />
					{:else}
						<Play size={14} />
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
</button>
