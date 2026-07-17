<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { getServerState, getModalState } from '$states';
	import { Button, Card } from '$components/ui';
	import {
		ServerCard,
		ServerDetailPanel,
		CreateServerModal,
		ImportServerModal
	} from '$components/servers';
	import { Plus, Server, Download } from 'lucide-svelte';
	import type { CreateServerData, ImportServerData, Server as ServerType } from '$types';

	const serverState = getServerState();
	const modal = getModalState();

	const servers = $derived(serverState.servers);
	const selectedServer = $derived(serverState.selectedServer);
	const loading = $derived(serverState.loading);
	const creationProgress = $derived(serverState.creationProgress);

	onMount(() => {
		serverState.loadServers();
		serverState.startPolling(15000);
	});

	onDestroy(() => {
		serverState.stopPolling();
	});

	function handleSelect(server: ServerType) {
		serverState.selectServer(server.id);
	}

	function handleStart(server: ServerType) {
		serverState.startServer(server.id);
	}

	function handleStop(server: ServerType) {
		serverState.stopServer(server.id);
	}

	async function handleCreate() {
		// Get suggested ports
		const allocatedPorts = new Set<number>();
		for (const s of servers) {
			allocatedPorts.add(s.game_port);
			allocatedPorts.add(s.query_port);
			allocatedPorts.add(s.rest_api_port);
		}

		let offset = 0;
		let suggestedPorts = { game_port: 8211, query_port: 27015, rest_api_port: 8212 };
		while (true) {
			const candidate = {
				game_port: 8211 + offset,
				query_port: 27015 + offset,
				rest_api_port: 8212 + offset
			};
			if (
				!allocatedPorts.has(candidate.game_port) &&
				!allocatedPorts.has(candidate.query_port) &&
				!allocatedPorts.has(candidate.rest_api_port)
			) {
				suggestedPorts = candidate;
				break;
			}
			offset++;
		}

		// @ts-ignore
		const result = await modal.showModal<CreateServerData | null>(CreateServerModal, {
			title: 'Create Server',
			suggestedPorts
		});

		if (result) {
			await serverState.createServer(result);
		}
	}

	async function handleImport() {
		// @ts-ignore
		const result = await modal.showModal<ImportServerData | null>(ImportServerModal, {
			title: 'Import Existing Server'
		});
		if (result) {
			await serverState.importServer(result);
		}
	}
</script>

<div class="flex h-full min-h-screen w-full gap-4 p-4">
	<!-- Server List Panel -->
	<div class="flex w-80 shrink-0 flex-col gap-4">
		<div class="flex items-center justify-between">
			<h2 class="text-primary-400 text-xl font-bold">Servers</h2>
			<div class="flex items-center gap-2">
				<Button
					variant="secondary"
					size="sm"
					class="flex items-center gap-2"
					onclick={handleImport}
				>
					<Download size={14} />
					Import
				</Button>
				<Button variant="primary" size="sm" class="flex items-center gap-2" onclick={handleCreate}>
					<Plus size={14} />
					New
				</Button>
			</div>
		</div>

		{#if creationProgress}
			<div class="bg-surface-800 flex items-center gap-3 rounded-sm p-3">
				<div
					class="border-secondary-400 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
				></div>
				<p class="text-surface-200 text-sm">{creationProgress}</p>
			</div>
		{/if}

		<div class="flex flex-col gap-2">
			{#if servers.length === 0 && !loading}
				<Card class="text-surface-400 text-center">
					<Server size={32} class="mx-auto mb-2 opacity-50" />
					<p>No servers configured</p>
					<p class="mt-1 text-sm">Create one to get started</p>
				</Card>
			{:else}
				{#each servers as server (server.id)}
					<ServerCard
						{server}
						selected={selectedServer?.id === server.id}
						onselect={handleSelect}
						onstart={handleStart}
						onstop={handleStop}
					/>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Detail Panel -->
	<div class="min-w-0 flex-1">
		{#if selectedServer}
			<ServerDetailPanel server={selectedServer} />
		{:else}
			<div class="text-surface-400 flex h-full items-center justify-center">
				<div class="text-center">
					<Server size={48} class="mx-auto mb-4 opacity-30" />
					<p class="text-lg">Select a server to view details</p>
				</div>
			</div>
		{/if}
	</div>
</div>
