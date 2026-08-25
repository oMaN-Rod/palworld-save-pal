<script lang="ts">
	import { page } from '$app/state';
	import { goto, replaceState } from '$app/navigation';
	import { cn } from '$theme';
	import { pluginsData } from '$lib/data';
	import { pluginEditor } from '$lib/plugins/pluginEditor.svelte';
	import {
		availableModes,
		leaveIsSafe,
		MODE_LABELS,
		resolveMode,
		type PaneMode
	} from '$lib/plugins/pluginPane';
	import { getModalState, getToastState } from '$states';
	import { Button } from '$components/ui';
	import type { PluginCommand } from '$types';
	import RunPane from '../components/RunPane.svelte';
	import RunResult from '../components/RunResult.svelte';

	const modal = getModalState();
	const toast = getToastState();

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

	function runCommand(command: PluginCommand, args: Record<string, unknown>) {
		if (!plugin) return;
		resultPluginId = plugin.id;
		if (command.destructive) {
			pendingApply = { commandId: command.id, args };
			pluginsData.run(plugin.id, command.id, args, true);
		} else {
			pendingApply = null;
			pluginsData.run(plugin.id, command.id, args, false);
		}
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
</script>

{#if !plugin}
	<p class="opacity-70">No such plugin.</p>
{:else}
	<div class="flex flex-col gap-3">
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
			{#if !plugin.bundled}
				<Button variant="ghost" size="sm" onclick={uninstall}>Uninstall</Button>
			{/if}
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

		{#if mode === 'run'}
			<RunPane {plugin} disabled={pluginsData.running !== null} onRun={runCommand} />
			{#if showResult && pluginsData.lastResult}
				<RunResult
					result={pluginsData.lastResult}
					pendingApply={showApplyFooter}
					onApply={applyPending}
					onCancel={cancelPending}
				/>
			{/if}
		{:else if mode === 'code'}
			{#await import('../components/CodePane.svelte') then { default: CodePane }}
				<CodePane id={plugin.id} />
			{/await}
		{/if}
	</div>
{/if}
