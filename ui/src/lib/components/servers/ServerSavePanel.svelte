<script lang="ts">
	import type { Server } from '$types';
	import { getServerState, getModalState } from '$states';
	import { Button, Card } from '$components/ui';
	import { FolderOpen, AlertTriangle } from 'lucide-svelte';
	import { goto } from '$app/navigation';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();
	const modal = getModalState();

	const isRunning = $derived(server.status?.running ?? false);

	async function handleLoadSave() {
		if (isRunning) {
			const confirmed = await modal.showConfirmModal({
				title: 'Server is Running',
				message: 'The server must be stopped before editing saves. Would you like to stop it now?',
				confirmText: 'Stop & Load',
				cancelText: 'Cancel'
			});
			if (!confirmed) return;
			await serverState.stopServer(server.id);
			// Wait a moment for container to stop
			await new Promise((r) => setTimeout(r, 3000));
		}
		await serverState.loadServerSave(server.id);
		goto('/edit');
	}
</script>

<div class="flex flex-col gap-4">
	<h3 class="text-lg font-bold">Save Files</h3>

	{#if isRunning}
		<Card class="border border-yellow-500/30">
			<div class="flex items-center gap-3 text-yellow-400">
				<AlertTriangle size={20} />
				<div>
					<p class="font-medium">Server is running</p>
					<p class="text-surface-400 text-sm">
						The server must be stopped before loading saves for editing. Editing while the server is
						running may corrupt save data.
					</p>
				</div>
			</div>
		</Card>
	{/if}

	<Card>
		<div class="flex items-center justify-between">
			<div>
				<h4 class="font-medium">Load Server Save</h4>
				<p class="text-surface-400 text-sm">
					Load this server's save files into the editor for viewing and modification.
				</p>
				<p class="text-surface-500 mt-1 text-xs">Save path: {server.saves_path}</p>
			</div>
			<Button variant="primary" onclick={handleLoadSave}>
				<FolderOpen size={14} />
				Load in Editor
			</Button>
		</div>
	</Card>
</div>
