<script lang="ts">
	import type { Server } from '$types';
	import { cn } from '$theme';
	import { Button, Card } from '$components/ui';
	import {
		Settings,
		Package,
		Terminal,
		FolderOpen,
		LayoutDashboard,
		Users,
		Play,
		Square,
		Trash2,
		Cpu,
		MemoryStick,
		Network,
		HardDrive
	} from 'lucide-svelte';
	import { getServerState, getModalState } from '$states';
	import ServerSettingsForm from './ServerSettingsForm.svelte';
	import ServerModsPanel from './ServerModsPanel.svelte';
	import ServerConsole from './ServerConsole.svelte';
	import ServerSavePanel from './ServerSavePanel.svelte';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();
	const modal = getModalState();

	type Tab = 'overview' | 'settings' | 'mods' | 'console' | 'saves';
	let activeTab: Tab = $state('overview');

	const isRunning = $derived(server.status?.running ?? false);
	const isNative = $derived(server.server_type === 'native');
	const stats = $derived(serverState.containerStats);

	// Poll stats when on overview tab and server is running
	let statsInterval: ReturnType<typeof setInterval> | null = null;

	$effect(() => {
		if (activeTab === 'overview' && isRunning && server.id) {
			serverState.loadStats(server.id);
			statsInterval = setInterval(() => {
				serverState.loadStats(server.id);
			}, 5000);
		} else {
			if (statsInterval) {
				clearInterval(statsInterval);
				statsInterval = null;
			}
			serverState.containerStats = null;
		}

		return () => {
			if (statsInterval) {
				clearInterval(statsInterval);
				statsInterval = null;
			}
		};
	});

	const statusColor = $derived.by(() => {
		const status = server.status?.status ?? 'not_found';
		switch (status) {
			case 'running':
				return 'text-green-400';
			case 'exited':
				return 'text-red-400';
			case 'created':
				return 'text-yellow-400';
			default:
				return 'text-gray-400';
		}
	});

	const tabs: { id: Tab; label: string; icon: typeof Settings }[] = [
		{ id: 'overview', label: 'Overview', icon: LayoutDashboard },
		{ id: 'settings', label: 'Settings', icon: Settings },
		{ id: 'mods', label: 'Mods', icon: Package },
		{ id: 'console', label: 'Console', icon: Terminal },
		{ id: 'saves', label: 'Saves', icon: FolderOpen }
	];

	async function handleDelete() {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Server',
			message: `Are you sure you want to delete "${server.name}"? This will stop the container and remove it. Server data on disk will be preserved.`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (confirmed) {
			await serverState.deleteServer(server.id);
		}
	}
</script>

<div class="flex h-full flex-col gap-4">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<div class="flex items-center gap-2">
				<h2 class="text-xl font-bold">{server.name}</h2>
				<span
					class={cn(
						'rounded-sm px-1.5 py-0.5 text-[10px] font-medium uppercase',
						isNative ? 'bg-blue-500/15 text-blue-400' : 'bg-cyan-500/15 text-cyan-400'
					)}
				>
					{isNative ? 'Native' : 'Docker'}
				</span>
			</div>
			<p class="text-surface-400 text-sm">{server.container_name}</p>
		</div>
		<div class="flex items-center gap-2">
			<Button
				variant="neutral"
				onclick={() =>
					isRunning ? serverState.stopServer(server.id) : serverState.startServer(server.id)}
			>
				{#if isRunning}
					<Square size={14} class={cn('mr-2 inline', 'text-red-400')} />
					Stop
				{:else}
					<Play size={14} class={cn('mr-2 inline', 'text-green-400')} />
					Start
				{/if}
			</Button>
			<Button variant="ghost" size="icon" onclick={handleDelete}>
				<Trash2 size={14} class="text-red-400" />
			</Button>
		</div>
	</div>

	<!-- Tabs -->
	<div class="border-surface-700 flex gap-1 border-b">
		{#each tabs as tab}
			<button
				class={cn(
					'flex items-center gap-2 px-4 py-2.5 text-sm font-medium transition-colors',
					activeTab === tab.id
						? 'border-b-2'
						: 'text-surface-400 hover:text-surface-200 border-b-2 border-transparent'
				)}
				onclick={() => (activeTab = tab.id)}
			>
				<tab.icon size={16} />
				{tab.label}
			</button>
		{/each}
	</div>

	<!-- Tab Content -->
	<div class="min-h-0 flex-1 overflow-y-auto">
		{#if activeTab === 'overview'}
			<div class="flex flex-col gap-4">
				<!-- Server info -->
				<div class="grid grid-cols-2 gap-4">
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">Status</h4>
						<p class={cn('text-lg font-bold capitalize', statusColor)}>
							{server.status?.status ?? 'Unknown'}
						</p>
					</Card>
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">Online Players</h4>
						<p class="flex items-center gap-2 text-lg font-bold">
							<Users size={18} class="text-green-400" />
							{server.player_count ?? 0} / {server.max_players}
						</p>
					</Card>
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">Total Players</h4>
						<p class="flex items-center gap-2 text-lg font-bold">
							<Users size={18} class="text-surface-400" />
							{server.total_players ?? 0}
						</p>
						{#if (server.total_players ?? 0) > 0}
							<p class="text-surface-400 text-xs">
								{Math.max(0, (server.total_players ?? 0) - (server.player_count ?? 0))} offline
							</p>
						{/if}
					</Card>
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">Game Port</h4>
						<p class="text-lg font-bold">{server.game_port}</p>
					</Card>
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">REST API Port</h4>
						<p class="text-lg font-bold">{server.rest_api_port}</p>
					</Card>
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">Server Name</h4>
						<p class="font-medium">{server.server_name}</p>
					</Card>
					<Card>
						<h4 class="text-surface-400 mb-1 text-xs font-medium uppercase">
							{isNative ? 'Install Path' : 'Image'}
						</h4>
						<p class="font-medium break-all">
							{isNative ? server.install_path : server.image_name}
						</p>
					</Card>
				</div>

				<!-- Container stats -->
				{#if isRunning && stats}
					<h4 class="text-surface-400 mt-2 text-xs font-medium uppercase">
						{isNative ? 'Process Resources' : 'Container Resources'}
					</h4>
					<div class="grid grid-cols-2 gap-4">
						<Card>
							<div class="flex items-center gap-2">
								<Cpu size={16} class="text-blue-400" />
								<h4 class="text-surface-400 text-xs font-medium uppercase">CPU</h4>
							</div>
							<p class="mt-1 text-2xl font-bold">{stats.cpu_percent}%</p>
						</Card>
						<Card>
							<div class="flex items-center gap-2">
								<MemoryStick size={16} class="text-purple-400" />
								<h4 class="text-surface-400 text-xs font-medium uppercase">Memory</h4>
							</div>
							<p class="mt-1 text-2xl font-bold">{stats.mem_percent}%</p>
							<p class="text-surface-400 text-xs">
								{stats.mem_usage_mb} MB / {stats.mem_limit_mb} MB
							</p>
						</Card>
						<Card>
							<div class="flex items-center gap-2">
								<Network size={16} class="text-green-400" />
								<h4 class="text-surface-400 text-xs font-medium uppercase">Network</h4>
							</div>
							<div class="mt-1 grid grid-cols-2 gap-2">
								<div>
									<p class="text-surface-400 text-[10px]">RX</p>
									<p class="text-sm font-bold">{stats.net_rx_mb} MB</p>
								</div>
								<div>
									<p class="text-surface-400 text-[10px]">TX</p>
									<p class="text-sm font-bold">{stats.net_tx_mb} MB</p>
								</div>
							</div>
						</Card>
						<Card>
							<div class="flex items-center gap-2">
								<HardDrive size={16} class="text-orange-400" />
								<h4 class="text-surface-400 text-xs font-medium uppercase">Disk I/O</h4>
							</div>
							<div class="mt-1 grid grid-cols-2 gap-2">
								<div>
									<p class="text-surface-400 text-[10px]">Read</p>
									<p class="text-sm font-bold">{stats.disk_read_mb} MB</p>
								</div>
								<div>
									<p class="text-surface-400 text-[10px]">Write</p>
									<p class="text-sm font-bold">{stats.disk_write_mb} MB</p>
								</div>
							</div>
						</Card>
					</div>
				{:else if isRunning}
					<Card class="text-surface-400 text-center text-sm">
						Loading {isNative ? 'process' : 'container'} stats...
					</Card>
				{/if}
			</div>
		{:else if activeTab === 'settings'}
			<ServerSettingsForm {server} />
		{:else if activeTab === 'mods'}
			<ServerModsPanel {server} />
		{:else if activeTab === 'console'}
			<ServerConsole {server} />
		{:else if activeTab === 'saves'}
			<ServerSavePanel {server} />
		{/if}
	</div>
</div>
