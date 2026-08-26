<script lang="ts">
	import { page } from '$app/state';
	import { goto, replaceState } from '$app/navigation';
	import { onDestroy, untrack } from 'svelte';
	import { cn } from '$theme';
	import { PLUGIN_WALL_CLOCK_LIMIT_SECONDS, pluginsData } from '$lib/data';
	import { pluginEditor } from '$lib/plugins/pluginEditor.svelte';
	import { slugify } from '$lib/plugins/pluginId';
	import {
		availableModes,
		leaveIsSafe,
		MODE_LABELS,
		resolveMode,
		type PaneMode
	} from '$lib/plugins/pluginPane';
	import { buildRunRequest, type ViewWidget } from '$lib/plugins/pluginView';
	import { PluginViewState } from '$lib/plugins/viewState.svelte';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Button, Tooltip } from '$components/ui';
	import { TextInputModal } from '$components';
	import type { PluginCommand } from '$types';
	import ApplyBar from '../components/ApplyBar.svelte';
	import PluginView from '../components/view/PluginView.svelte';
	import RunPane from '../components/RunPane.svelte';
	import RunResult from '../components/RunResult.svelte';

	const modal = getModalState();
	const toast = getToastState();
	const appState = getAppState();

	const plugin = $derived(pluginsData.plugins.find((p) => p.id === page.params.id));

	// `page.url` doesn't refresh on `replaceState`, so the active mode is tracked locally
	// and mirrored to the URL; any real navigation hands authority back to the URL.
	let modeOverride: PaneMode | null = $state(null);
	$effect(() => {
		page.url.href;
		modeOverride = null;
	});
	const mode = $derived(modeOverride ?? resolveMode(page.url.searchParams.get('mode'), plugin));

	let pendingApply: { commandId: string; args: Record<string, unknown> } | null = $state(null);
	let resultPluginId: string | null = $state(null);

	let view: PluginViewState | null = $state(null);
	let viewPluginId: string | null = $state(null);

	let elapsedSeconds = $state(0);
	let elapsedTimer: ReturnType<typeof setInterval> | undefined;

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
		const id = plugin?.id ?? null;
		if (id === untrack(() => viewPluginId)) return;
		viewPluginId = id;
		const current = plugin;
		if (!current) {
			view = null;
			return;
		}
		const next = new PluginViewState(current.ui, current.commands);
		view = next;
		if (next.hasView) next.loadEntities();
	});

	$effect(() => {
		appState.saveFile?.name;
		const active = untrack(() => view);
		if (!active) return;
		active.loadEntities();
	});

	let recordedRunId: string | null = $state(null);

	$effect(() => {
		const result = pluginsData.lastResult;
		const active = view;
		const ran = lastRun;
		if (!result || !active || !ran || result.status !== 'ok') return;
		if (plugin === undefined || resultPluginId !== plugin.id) return;
		if (untrack(() => recordedRunId) === result.run_id) return;
		recordedRunId = result.run_id;
		active.recordResult(ran.commandId, result.result);
	});

	const showApplyFooter = $derived(
		pendingApply !== null && pluginsData.lastResult?.status === 'ok'
	);
	const showResult = $derived(
		plugin !== undefined && resultPluginId === plugin.id && pluginsData.lastResult !== null
	);

	async function selectMode(next: PaneMode) {
		if (mode === 'code' && next !== 'code' && !leaveIsSafe(pluginEditor, null)) {
			const confirmed = await modal.showConfirmModal({
				title: 'Discard unsaved changes?',
				message: `"${pluginEditor.pluginId}" has unsaved edits. Switching tabs will discard them.`,
				confirmText: 'Discard',
				cancelText: 'Cancel'
			});
			if (!confirmed) return;
		}
		modeOverride = next;
		const url = new URL(page.url);
		if (next === 'run') {
			url.searchParams.delete('mode');
		} else {
			url.searchParams.set('mode', next);
		}
		replaceState(url, page.state);
	}

	async function uninstall() {
		if (!plugin) return;
		// By value: `plugin` is derived from the list `uninstall` itself reloads, so it
		// reads `undefined` the moment the await resolves.
		const { id, name } = plugin;
		const confirmed = await modal.showConfirmModal({
			title: `Uninstall "${name}"?`,
			confirmText: 'Uninstall',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		try {
			await pluginsData.uninstall(id);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Uninstall failed', 'error');
			return;
		}
		toast.add(`Uninstalled ${name}.`, 'Plugin', 'success');
		if (pluginEditor.pluginId === id) pluginEditor.reset();
		await goto('/plugins');
	}

	function decodeBase64(content: string): Uint8Array<ArrayBuffer> {
		const binary = atob(content);
		const bytes = new Uint8Array(binary.length);
		for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
		return bytes as Uint8Array<ArrayBuffer>;
	}

	function browserDownload(name: string, bytes: Uint8Array<ArrayBuffer>): void {
		const blob = new Blob([bytes], { type: 'application/octet-stream' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = name;
		a.click();
		URL.revokeObjectURL(url);
	}

	async function exportPlugin() {
		if (!plugin) return;
		try {
			const result = await pluginsData.exportPlugin(plugin.id);
			if (Array.isArray(result)) {
				for (const file of result) browserDownload(file.name, decodeBase64(file.content));
				toast.add(`Exported ${plugin.name}.`, 'Plugin', 'success');
				return;
			}
			toast.add(result.message, 'Plugin', 'success');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Export failed', 'error');
		}
	}

	function uniqueCloneId(baseId: string): string {
		const ids = new Set(pluginsData.plugins.map((item) => item.id));
		if (!ids.has(baseId)) return baseId;
		let index = 2;
		while (ids.has(`${baseId}-${index}`)) index += 1;
		return `${baseId}-${index}`;
	}

	async function clonePlugin() {
		if (!plugin) return;
		// @ts-expect-error -- TextInputModal's `closeModal` prop is injected by Modal.svelte
		const targetName = await modal.showModal<string>(TextInputModal, {
			title: `Clone "${plugin.name}"`,
			value: `${plugin.name} Copy`,
			inputLabel: 'Name'
		});
		if (!targetName) return;

		const baseId = slugify(targetName);
		if (!baseId) {
			toast.add('Plugin name must contain at least one letter or digit.', 'Plugin', 'error');
			return;
		}

		const targetId = uniqueCloneId(baseId);
		try {
			const cloned = await pluginsData.clonePlugin(plugin.id, targetId, targetName.trim());
			toast.add(`Cloned ${plugin.name} to ${cloned.name}.`, 'Plugin', 'success');
			await goto(`/plugins/${encodeURIComponent(cloned.id)}?mode=code`);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Clone failed', 'error');
		}
	}

	let lastRun: { commandId: string } | null = $state(null);

	function runCommand(command: PluginCommand, args: Record<string, unknown>) {
		if (!plugin) return;
		resultPluginId = plugin.id;
		lastRun = { commandId: command.id };
		if (command.destructive) {
			pendingApply = { commandId: command.id, args };
			pluginsData.run(plugin.id, command.id, args, true);
		} else {
			pendingApply = null;
			pluginsData.run(plugin.id, command.id, args, false);
		}
	}

	function runFromView(widget: ViewWidget) {
		if (!plugin || !view) return;
		const command = plugin.commands.find((c) => c.id === widget.command);
		if (!command) return;
		const { args } = buildRunRequest(widget, command, view.runtime());
		runCommand(command, args);
	}

	function applyPending() {
		if (!pendingApply || !plugin) return;
		const { commandId, args } = pendingApply;
		pendingApply = null;
		pluginsData.run(plugin.id, commandId, args, false);
	}

	function cancelPending() {
		pendingApply = null;
		pluginsData.lastResult = null;
	}

	onDestroy(() => {
		if (elapsedTimer) clearInterval(elapsedTimer);
	});
</script>

{#if !plugin}
	<p class="opacity-70">No such plugin.</p>
{:else}
	<div class="flex h-full flex-col gap-3">
		<div class="flex items-start justify-between gap-4">
			<div class="min-w-0">
				<div class="flex items-center gap-2">
					<h2 class="truncate text-lg font-semibold">{plugin.name}</h2>
					<span class="text-surface-400 text-xs">v{plugin.version}</span>
					<span
						class={cn(
							'rounded-full px-2 py-0.5 text-xs',
							plugin.bundled
								? 'bg-secondary-500/25 text-secondary-300'
								: 'bg-primary-500/25 text-primary-300'
						)}
					>
						{plugin.bundled ? 'Bundled' : 'User'}
					</span>
				</div>
				{#if plugin.author}
					<div class="text-surface-400 text-xs">by {plugin.author}</div>
				{/if}
				{#if plugin.error}
					<div class="text-error-500 text-xs">{plugin.error}</div>
				{/if}
			</div>

			<div class="flex items-center gap-2">
				<Button variant="ghost" size="sm" onclick={exportPlugin}>Export</Button>
				<Button variant="ghost" size="sm" onclick={clonePlugin}>Clone</Button>
				{#if !plugin.bundled}
					<Button variant="ghost" size="sm" onclick={uninstall}>Uninstall</Button>
				{/if}
			</div>
		</div>

		<div class="border-surface-700 flex gap-1 border-b" role="tablist" aria-label="Plugin pane">
			{#each availableModes(plugin) as paneMode (paneMode)}
				<button
					type="button"
					role="tab"
					aria-selected={mode === paneMode}
					class={cn(
						'-mb-px border-b-2 px-3 py-1.5 text-sm',
						mode === paneMode
							? 'border-primary-500 text-surface-50 font-medium'
							: 'text-surface-400 hover:text-surface-200 border-transparent'
					)}
					onclick={() => selectMode(paneMode)}
				>
					{MODE_LABELS[paneMode]}
				</button>
			{/each}
		</div>
		<div class="flex flex-col gap-3">
			{#if pluginsData.running}
				<div class="border-surface-700 flex items-center justify-between rounded-sm border p-2">
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
				</div>
			{/if}
		</div>

		{#if mode === 'run'}
			{#if view?.hasView}
				<PluginView
					state={view}
					commands={plugin.commands}
					disabled={!plugin.enabled || pluginsData.running !== null}
					onRun={runFromView}
				>
					{#if showApplyFooter}
						<ApplyBar
							summary={pluginsData.lastResult?.summary ?? null}
							onApply={applyPending}
							onCancel={cancelPending}
						/>
					{/if}
					{#if showResult && pluginsData.lastResult}
						<RunResult result={pluginsData.lastResult} />
					{/if}
				</PluginView>
			{:else}
				<div class="grid grid-cols-[25%_1fr] gap-2">
					<RunPane {plugin} disabled={pluginsData.running !== null} onRun={runCommand} />
					{#if showResult && pluginsData.lastResult}
						<RunResult
							result={pluginsData.lastResult}
							pendingApply={showApplyFooter}
							onApply={applyPending}
							onCancel={cancelPending}
						/>
					{:else}
						<div class="flex h-full flex-col items-center justify-center gap-2 text-center">
							<p class="opacity-70">Run a command to see the result here.</p>
						</div>
					{/if}
				</div>
			{/if}
		{:else if mode === 'code'}
			{#await import('../components/CodePane.svelte') then { default: CodePane }}
				<CodePane id={plugin.id} />
			{/await}
		{/if}
	</div>
{/if}
