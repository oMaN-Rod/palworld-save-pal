<script lang="ts">
	import { Button, Card, Input, Tooltip } from '$components/ui';
	import { Save, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import type { ImportServerData } from '$types';

	let { title = 'Import Existing Server', closeModal } = $props<{
		title?: string;
		closeModal: (value: ImportServerData | null) => void;
	}>();

	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let modalContainer: HTMLDivElement;
	let installPath = $state('');
	let name = $state('');
	let queryPort = $state(27015);
	let launchArgs = $state('');
	let workshopDir = $state('');

	const canSubmit = $derived(isDesktopMode || installPath.trim().length > 0);

	function handleSubmit() {
		if (!canSubmit) return;
		const data: ImportServerData = {
			install_path: isDesktopMode ? '__select__' : installPath.trim(),
			name: name.trim(),
			query_port: queryPort,
			launch_args: launchArgs,
			workshop_dir: workshopDir
		};
		closeModal(data);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="max-w-[600px] min-w-[520px]">
		<h3 class="h3 mb-4">{title}</h3>

		<div class="flex flex-col gap-3">
			{#if isDesktopMode}
				<p class="text-surface-300 text-sm">
					A folder picker will open when you click Import. Choose the folder that contains
					<span class="font-mono">PalServer.exe</span>.
				</p>
			{:else}
				<Input
					label="Server Folder (contains PalServer.exe)"
					bind:value={installPath}
					placeholder="C:\PalworldServers\MyWorld"
				/>
			{/if}

			<Input
				label="Display Name (optional)"
				bind:value={name}
				placeholder="Detected from server config"
			/>
			<Input label="Query Port" type="number" bind:value={queryPort} />
			<Input
				label="Extra Launch Args (optional)"
				bind:value={launchArgs}
				placeholder="-publiclobby -NumberOfWorkerThreadsServer=8"
			/>
			<Input
				label="Steam Workshop Dir (optional, auto-detected)"
				bind:value={workshopDir}
				placeholder="Leave empty to auto-detect"
			/>

			<p class="text-surface-400 text-xs">
				Import does not modify, move, or download any server files.
			</p>
		</div>

		<div class="mt-4 flex justify-end gap-2">
			<Tooltip position="bottom">
				{#snippet children()}
					<Button
						variant="ghost"
						size="icon"
						onclick={handleSubmit}
						disabled={!canSubmit}
						data-modal-primary
					>
						<Save />
					</Button>
				{/snippet}
				{#snippet popup()}
					<span>Import</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				{#snippet children()}
					<Button variant="ghost" size="icon" onclick={() => closeModal(null)}>
						<X />
					</Button>
				{/snippet}
				{#snippet popup()}
					<span>Cancel</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
