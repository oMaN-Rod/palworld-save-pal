<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { pluginsData, PLUGIN_WALL_CLOCK_LIMIT_SECONDS } from '$lib/data';
	import { pluginEditor } from '$lib/plugins/pluginEditor.svelte';
	import { slugify } from '$lib/plugins/pluginId';
	import { getModalState, getToastState, getAppState } from '$states';
	import { Button, FileDropzone, Tooltip } from '$components/ui';
	import { TextInputModal } from '$components';
	import type { PluginCommand } from '$types';
	import PluginCard from './components/PluginCard.svelte';
	import RunResult from './components/RunResult.svelte';

	const modal = getModalState();
	const toast = getToastState();
	const appState = getAppState();

	let installFiles: FileList | undefined = $state();
	let pendingApply: { pluginId: string; commandId: string; args: Record<string, unknown> } | null =
		$state(null);
	let elapsedSeconds = $state(0);
	let elapsedTimer: ReturnType<typeof setInterval> | undefined;

	onMount(() => {
		pluginsData.list();
	});

	onDestroy(() => {
		if (elapsedTimer) clearInterval(elapsedTimer);
	});

	$effect(() => {
		if (pluginsData.running) {
			elapsedSeconds = 0;
			elapsedTimer = setInterval(() => {
				elapsedSeconds += 1;
			}, 1000);
		} else if (elapsedTimer) {
			clearInterval(elapsedTimer);
			elapsedTimer = undefined;
		}
	});

	$effect(() => {
		if (installFiles && installFiles.length > 0) {
			const files = installFiles;
			installFiles = undefined;
			installFile(files);
		}
	});

	function fileToBase64(file: File): Promise<string> {
		return new Promise((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => {
				const result = reader.result as string;
				resolve(result.slice(result.indexOf(',') + 1));
			};
			reader.onerror = () => reject(reader.error);
			reader.readAsDataURL(file);
		});
	}

	async function installFile(files: FileList) {
		const file = files[0];
		if (!file) return;
		try {
			const content = await fileToBase64(file);
			const summary = await pluginsData.install(file.name, content);
			toast.add(`Installed ${summary.name}.`, 'Plugin', 'success');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Install failed', 'error');
		}
	}

	function toggleEnabled(id: string, enabled: boolean) {
		pluginsData.setEnabled(id, enabled);
	}

	async function editPlugin(id: string) {
		await goto(`/plugins/editor?id=${encodeURIComponent(id)}`);
	}

	async function newPlugin() {
		// @ts-expect-error -- TextInputModal's `closeModal` prop is injected by Modal.svelte
		const name = await modal.showModal<string>(TextInputModal, {
			title: 'New plugin',
			inputLabel: 'Name'
		});
		if (!name) return;
		const id = slugify(name);
		if (!id) {
			toast.add('Plugin name must contain at least one letter or digit.', 'Plugin', 'error');
			return;
		}
		try {
			const createdId = await pluginEditor.create(id, name);
			await pluginsData.list();
			await editPlugin(createdId);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Could not create plugin', 'error');
		}
	}

	async function uninstall(id: string, name: string) {
		const confirmed = await modal.showConfirmModal({
			title: `Uninstall "${name}"?`,
			confirmText: 'Uninstall',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		try {
			await pluginsData.uninstall(id);
			toast.add(`Uninstalled ${name}.`, 'Plugin', 'success');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Uninstall failed', 'error');
		}
	}

	function runCommand(pluginId: string, command: PluginCommand, args: Record<string, unknown>) {
		if (command.destructive) {
			pendingApply = { pluginId, commandId: command.id, args };
			pluginsData.run(pluginId, command.id, args, true);
		} else {
			pendingApply = null;
			pluginsData.run(pluginId, command.id, args, false);
		}
	}

	function applyPending() {
		if (!pendingApply) return;
		const { pluginId, commandId, args } = pendingApply;
		pendingApply = null;
		pluginsData.run(pluginId, commandId, args, false);
	}

	function cancelPending() {
		pendingApply = null;
		pluginsData.lastResult = null;
	}

	const showApplyFooter = $derived(
		pendingApply !== null && pluginsData.lastResult?.status === 'ok'
	);
</script>

<div class="animate-fade-in flex h-full flex-col gap-4 overflow-y-auto p-4">
	<div class="flex items-center justify-between">
		<h1 class="text-xl font-semibold">Plugins</h1>
		<Button size="sm" onclick={newPlugin}>New plugin</Button>
	</div>

	<FileDropzone name="plugin-install" accept=".lua,.zip" bind:files={installFiles}>
		{#snippet message()}
			<h3 class="h3">Install a plugin</h3>
			<span>Drag and drop a .lua file or a .zip archive here</span>
		{/snippet}
	</FileDropzone>

	{#if pluginsData.running}
		<div class="border-surface-700 flex items-center justify-between rounded-sm border p-3">
			<div class="flex flex-col">
				<span class="text-sm">
					Running <span class="font-medium">{pluginsData.running.commandId}</span> on
					<span class="font-medium">{pluginsData.running.pluginId}</span>...
				</span>
				<span class="text-surface-400 text-xs">
					{elapsedSeconds}s elapsed -- stops automatically after {PLUGIN_WALL_CLOCK_LIMIT_SECONDS}s
				</span>
				{#if appState.progressMessage}
					<span class="text-surface-400 text-xs">{appState.progressMessage}</span>
				{/if}
			</div>
			<Tooltip
				label="Cancel can't be delivered while this connection is busy running the command -- wait for it to finish or time out."
			>
				<Button variant="ghost" size="sm" disabled>Cancel</Button>
			</Tooltip>
		</div>
	{/if}

	{#if pluginsData.lastResult}
		<RunResult
			result={pluginsData.lastResult}
			pendingApply={showApplyFooter}
			onApply={applyPending}
			onCancel={cancelPending}
		/>
	{/if}

	{#if pluginsData.plugins.length === 0}
		<p class="opacity-70">No plugins installed yet. Drop a .lua file or a .zip archive above.</p>
	{:else}
		<div class="flex flex-col gap-3">
			{#each pluginsData.plugins as plugin (plugin.id)}
				<PluginCard
					{plugin}
					disabled={pluginsData.running !== null}
					onToggleEnabled={(enabled) => toggleEnabled(plugin.id, enabled)}
					onUninstall={() => uninstall(plugin.id, plugin.name)}
					onEdit={() => editPlugin(plugin.id)}
					onRun={(command, args) => runCommand(plugin.id, command, args)}
				/>
			{/each}
		</div>
	{/if}
</div>
