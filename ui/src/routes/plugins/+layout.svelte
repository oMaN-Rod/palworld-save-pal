<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { beforeNavigate, goto } from '$app/navigation';
	import { fade } from 'svelte/transition';
	import { pluginsData, PLUGIN_WALL_CLOCK_LIMIT_SECONDS } from '$lib/data';
	import { pluginEditor } from '$lib/plugins/pluginEditor.svelte';
	import { leaveIsSafe, pluginIdFromPath } from '$lib/plugins/pluginPane';
	import { slugify } from '$lib/plugins/pluginId';
	import { getModalState, getToastState, getAppState } from '$states';
	import { Button, FileDropzone, Tooltip } from '$components/ui';
	import { TextInputModal } from '$components';
	import PluginList from './components/PluginList.svelte';

	const { children } = $props();

	const modal = getModalState();
	const toast = getToastState();
	const appState = getAppState();

	let installFiles: FileList | undefined = $state();
	let elapsedSeconds = $state(0);
	let elapsedTimer: ReturnType<typeof setInterval> | undefined;

	const selectedId = $derived(page.params.id);

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

	let discardConfirmed = false;

	beforeNavigate(async (navigation) => {
		if (discardConfirmed) {
			discardConfirmed = false;
			return;
		}
		if (leaveIsSafe(pluginEditor, pluginIdFromPath(navigation.to?.url.pathname))) return;
		navigation.cancel();
		const target = navigation.to?.url;
		// No target means the tab itself is closing; cancelling is what raises the
		// browser's own prompt, and nothing here can replace it.
		if (!target) return;
		const confirmed = await modal.showConfirmModal({
			title: 'Discard unsaved changes?',
			message: `"${pluginEditor.pluginId}" has unsaved edits. Leaving will discard them.`,
			confirmText: 'Discard',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		discardConfirmed = true;
		await goto(target);
	});

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
			await goto(`/plugins/${encodeURIComponent(createdId)}?mode=code`);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Could not create plugin', 'error');
		}
	}
</script>

<div class="flex h-full w-full flex-col overflow-hidden">
	<div class="mx-2 my-2 flex flex-col gap-3">
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
	</div>

	<div class="flex flex-1 overflow-hidden">
		<aside class="border-surface-700 w-72 shrink-0 overflow-y-auto border-r p-3">
			{#if pluginsData.plugins.length === 0}
				<p class="opacity-70">
					No plugins installed yet. Drop a .lua file or a .zip archive above.
				</p>
			{:else}
				<PluginList plugins={pluginsData.plugins} {selectedId} onToggleEnabled={toggleEnabled} />
			{/if}
		</aside>
		<div class="relative flex-1 overflow-hidden">
			{#key page.url.pathname}
				<div
					class="absolute inset-0 overflow-y-auto p-4"
					transition:fade={{ duration: 150 }}
					onoutrostart={(event) => event.currentTarget.classList.add('pointer-events-none')}
				>
					{@render children()}
				</div>
			{/key}
		</div>
	</div>
</div>
