<script lang="ts">
	import type * as MonacoE from 'monaco-editor';
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { Button, Input, Monaco, Spinner } from '$components/ui';
	import { buildEditorTheme, EDITOR_THEME_NAME } from '$components/ui/monaco/paletteTheme';
	import { pluginsData } from '$lib/data';
	import { MANIFEST_PATH, pluginEditor } from '$lib/plugins/pluginEditor.svelte';
	import { registerLuaProviders, type ApiSnapshot } from '$lib/plugins/luaProviders';
	import { getModalState, getToastState, theme, type ThemeName } from '$states';
	import { persistedState } from 'svelte-persisted-state';
	import Columns2 from '@lucide/svelte/icons/columns-2';
	import Rows2 from '@lucide/svelte/icons/rows-2';
	import X from '@lucide/svelte/icons/x';
	import RunResult from '../components/RunResult.svelte';
	import ResizableSplit from './components/ResizableSplit.svelte';
	import RunButton from './components/RunButton.svelte';
	import {
		clampSplitRatio,
		DEFAULT_SPLIT_RATIO,
		toggleOrientation,
		type SplitOrientation
	} from './components/resizableSplit';

	const toast = getToastState();
	const modal = getModalState();

	let newFilePath = $state('');
	const newFileError = $derived(newFilePath.trim() ? pluginEditor.validNewPath(newFilePath) : null);

	const splitOrientation = persistedState<SplitOrientation>(
		'psp-plugin-editor-split-orientation',
		'horizontal'
	);
	const splitRatio = persistedState<number>('psp-plugin-editor-split-ratio', DEFAULT_SPLIT_RATIO);

	const LIGHT_THEMES = new Set<ThemeName>(['light', 'lamball']);
	const CHECK_DEBOUNCE_MS = 400;

	let paletteProbe: HTMLElement | undefined = $state();
	let editorThemeData = $state<MonacoE.editor.IStandaloneThemeData>();
	let monaco = $state<typeof MonacoE | undefined>();
	let editor = $state<MonacoE.editor.IStandaloneCodeEditor | undefined>();
	let providers: { dispose(): void } | undefined;
	let checkTimer: ReturnType<typeof setTimeout> | undefined;

	const pluginId = $derived(page.url.searchParams.get('id'));
	const language = $derived(pluginEditor.activePath === MANIFEST_PATH ? 'json' : 'lua');
	const snapshot: ApiSnapshot = $derived({
		definition: pluginEditor.definition ?? { globals: [], handles: [] },
		granted: pluginEditor.granted
	});

	$effect(() => {
		const current = theme.current;
		if (!paletteProbe) return;
		const style = getComputedStyle(paletteProbe);
		editorThemeData = buildEditorTheme(
			(name) => style.getPropertyValue(name),
			LIGHT_THEMES.has(current)
		);
	});

	onMount(async () => {
		splitRatio.current = clampSplitRatio(splitRatio.current);
		if (!pluginId) {
			await goto('/plugins');
			return;
		}
		await pluginEditor.loadDefinition();
		try {
			await pluginEditor.open(pluginId);
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Could not open plugin', 'error');
			await goto('/plugins');
		}
	});

	onDestroy(() => {
		if (checkTimer) clearTimeout(checkTimer);
		providers?.dispose();
		pluginEditor.reset();
	});

	function onSourceChanged(text: string) {
		pluginEditor.setSource(pluginEditor.activePath, text);
		if (checkTimer) clearTimeout(checkTimer);
		checkTimer = setTimeout(() => {
			pluginEditor.checkActive();
		}, CHECK_DEBOUNCE_MS);
	}

	function onEditorReady(instance: MonacoE.editor.IStandaloneCodeEditor) {
		if (!monaco) return;
		providers?.dispose();
		providers = registerLuaProviders(monaco, () => snapshot);
		instance.getModel()?.onDidChangeContent(() => onSourceChanged(instance.getValue()));
	}

	$effect(() => {
		const model = editor?.getModel();
		if (!monaco || !model) return;
		const markers: MonacoE.editor.IMarkerData[] = [];
		if (pluginEditor.activePath === MANIFEST_PATH) {
			if (pluginEditor.manifestError) {
				markers.push({
					severity: monaco.MarkerSeverity.Error,
					message: pluginEditor.manifestError,
					startLineNumber: 1,
					endLineNumber: 1,
					startColumn: 1,
					endColumn: model.getLineMaxColumn(1)
				});
			}
		} else if (pluginEditor.syntaxError) {
			const line = Math.min(pluginEditor.syntaxError.line ?? 1, model.getLineCount());
			markers.push({
				severity: monaco.MarkerSeverity.Error,
				message: pluginEditor.syntaxError.message,
				startLineNumber: line,
				endLineNumber: line,
				startColumn: 1,
				endColumn: model.getLineMaxColumn(line)
			});
		}
		monaco.editor.setModelMarkers(model, 'psp', markers);
	});

	const showApplyFooter = $derived(
		pluginEditor.pendingApply !== null && pluginsData.lastResult?.status === 'ok'
	);

	async function save() {
		try {
			await pluginEditor.saveActive();
			toast.add(`Saved ${pluginEditor.activePath}.`, 'Plugin', 'success');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Save failed', 'error');
		}
	}

	function submitNewFile(event: SubmitEvent) {
		event.preventDefault();
		if (newFileError !== null || !newFilePath.trim()) return;
		pluginEditor.addFile(newFilePath.trim());
		newFilePath = '';
	}

	async function removeFile(path: string) {
		const confirmed = await modal.showConfirmModal({
			title: `Delete "${path}"?`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;
		try {
			await pluginEditor.deleteFile(path);
			toast.add(`Deleted ${path}.`, 'Plugin', 'success');
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Delete failed', 'error');
		}
	}
</script>

<!-- Palette source for the editor theme; its own data-theme makes the read
     independent of the layout's <body> theme effect. Never visible. -->
<div
	bind:this={paletteProbe}
	data-theme={theme.current}
	class="palette-probe"
	aria-hidden="true"
></div>

{#if pluginEditor.loading}
	<Spinner />
{:else}
	<div class="flex h-full flex-col gap-2 p-2">
		<div class="flex items-center gap-2">
			<Button variant="ghost" size="sm" onclick={() => goto('/plugins')}>Back</Button>
			<span class="font-bold">{pluginEditor.pluginId}</span>
			{#if pluginEditor.bundled}
				<span class="bg-secondary-500/25 text-secondary-300 rounded-full px-2 py-0.5 text-xs">
					Bundled — read only
				</span>
			{/if}
			{#if !pluginEditor.enabled}
				<span class="bg-warning-500/25 text-warning-300 rounded-full px-2 py-0.5 text-xs">
					Disabled in the panel; drafts still run here
				</span>
			{/if}
			<div class="grow"></div>
			<Button
				variant="ghost"
				size="sm"
				aria-label="Toggle split orientation"
				onclick={() => (splitOrientation.current = toggleOrientation(splitOrientation.current))}
			>
				{#if splitOrientation.current === 'horizontal'}
					<Columns2 class="h-4 w-4" />
				{:else}
					<Rows2 class="h-4 w-4" />
				{/if}
			</Button>
			<RunButton
				commands={pluginEditor.commands}
				running={pluginsData.running !== null}
				bundled={pluginEditor.bundled}
				onRun={(commandId, dryRun) => pluginEditor.runDraft(commandId, {}, dryRun)}
			/>
			<Button
				size="sm"
				disabled={pluginEditor.bundled ||
					pluginEditor.saving ||
					!pluginEditor.isDirty(pluginEditor.activePath)}
				onclick={save}
			>
				Save
			</Button>
		</div>

		<div class="flex flex-wrap items-center gap-1">
			{#each pluginEditor.paths as path (path)}
				<div class="flex items-center">
					<Button
						variant={pluginEditor.activePath === path ? 'secondary' : 'ghost'}
						size="sm"
						onclick={() => pluginEditor.selectPath(path)}
					>
						{path}{pluginEditor.isDirty(path) ? ' •' : ''}
					</Button>
					{#if !pluginEditor.bundled && path !== MANIFEST_PATH && path !== pluginEditor.entry}
						<Button
							variant="ghost"
							size="icon"
							aria-label={`Delete ${path}`}
							onclick={() => removeFile(path)}
						>
							<X class="h-3 w-3" />
						</Button>
					{/if}
				</div>
			{/each}
			{#if !pluginEditor.bundled}
				<form class="flex items-center gap-1" onsubmit={submitNewFile}>
					<Input
						bind:value={newFilePath}
						placeholder="lib/new.lua"
						inputClass="my-0 py-1"
						error={newFileError !== null}
					/>
					<Button
						type="submit"
						variant="ghost"
						size="sm"
						disabled={newFilePath.trim() === '' || newFileError !== null}
					>
						New file
					</Button>
				</form>
				{#if newFileError}
					<span class="text-error-400 text-xs">{newFileError}</span>
				{/if}
			{/if}
		</div>

		{#if pluginEditor.warnings.length > 0}
			<div class="bg-warning-500/15 text-warning-300 flex flex-col gap-1 rounded p-2 text-xs">
				{#each pluginEditor.warnings as warning (warning.kind + warning.name)}
					<span>{warning.message}</span>
				{/each}
			</div>
		{/if}

		<div class="min-h-0 grow">
			<ResizableSplit orientation={splitOrientation.current} bind:ratio={splitRatio.current}>
				{#snippet a()}
					<Monaco
						bind:monaco
						bind:editor
						value={pluginEditor.activeText}
						{language}
						theme={EDITOR_THEME_NAME}
						themeData={editorThemeData}
						onready={onEditorReady}
					/>
				{/snippet}
				{#snippet b()}
					<div class="grid h-full grid-rows-1 overflow-auto">
						{#if pluginsData.lastResult}
							<RunResult
								result={pluginsData.lastResult}
								pendingApply={showApplyFooter}
								onApply={() => pluginEditor.applyPending()}
								onCancel={() => pluginEditor.cancelPending()}
							/>
						{:else}
							<div
								class="text-surface-400 flex h-full items-center justify-center p-4 text-center text-sm"
							>
								Run a draft to see results here.
							</div>
						{/if}
					</div>
				{/snippet}
			</ResizableSplit>
		</div>
	</div>
{/if}

<style>
	.palette-probe {
		position: absolute;
		width: 0;
		height: 0;
		overflow: hidden;
		visibility: hidden;
		pointer-events: none;
	}
</style>
