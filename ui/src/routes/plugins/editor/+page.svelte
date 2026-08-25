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
	import {
		lspClient,
		rangeFromLsp,
		type DiagnosticSeverity,
		type LspDiagnostic,
		type LspRange,
		type TierStatus
	} from '$lib/plugins/lspClient';
	import {
		toCompletionSuggestions,
		toMonacoHover,
		type LspCompletionKind
	} from '$lib/plugins/lspMonaco';
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
	const TIER_PROBE_INTERVAL_MS = 2000;
	const MAX_TIER_PROBE_BACKOFF_MS = 30000;

	let paletteProbe: HTMLElement | undefined = $state();
	let editorThemeData = $state<MonacoE.editor.IStandaloneThemeData>();
	let monaco = $state<typeof MonacoE | undefined>();
	let editor = $state<MonacoE.editor.IStandaloneCodeEditor | undefined>();
	let providers: { dispose(): void } | undefined;
	let lspProviders: { dispose(): void }[] = [];
	let checkTimer: ReturnType<typeof setTimeout> | undefined;
	let tierProbeTimer: ReturnType<typeof setTimeout> | undefined;
	let tierProbeDelayMs = TIER_PROBE_INTERVAL_MS;
	let destroyed = false;
	let fullTierLive = $state(false);
	let lspDiagnosticsByPath = $state<Record<string, LspDiagnostic[]>>({});

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

	function registerDiagnosticsListener(): void {
		lspClient.onDiagnostics((uri, diagnostics) => {
			const path = lspClient.pathFor(uri);
			if (path === null) return;
			lspDiagnosticsByPath = { ...lspDiagnosticsByPath, [path]: diagnostics };
		});
	}

	function scheduleTierProbe(id: string, delayMs: number): void {
		if (destroyed) return;
		if (tierProbeTimer) clearTimeout(tierProbeTimer);
		tierProbeTimer = setTimeout(() => void probeTier(id), delayMs);
	}

	function backOffTierProbe(): void {
		tierProbeDelayMs = Math.min(tierProbeDelayMs * 2, MAX_TIER_PROBE_BACKOFF_MS);
	}

	/**
	 * A rejection this deep almost always means the language server process is
	 * gone, not a routine LSP error reply, so this both downgrades and tries to
	 * recover: a stale session left open would keep answering to a dead root.
	 */
	function onLspRequestFailed(e: unknown): void {
		if (!fullTierLive || !pluginId) return;
		fullTierLive = false;
		lspDiagnosticsByPath = {};
		console.error('the plugin language server stopped responding', e);
		lspClient.dispose();
		scheduleTierProbe(pluginId, tierProbeDelayMs);
		backOffTierProbe();
	}

	async function activateFullTier(id: string): Promise<void> {
		try {
			const sources = Object.fromEntries(
				Object.entries(pluginEditor.files).filter(([path]) => path !== MANIFEST_PATH)
			);
			await lspClient.open(id, sources);
			if (destroyed) {
				lspClient.dispose();
				return;
			}
			registerDiagnosticsListener();
			fullTierLive = true;
			tierProbeDelayMs = TIER_PROBE_INTERVAL_MS;
		} catch (e) {
			console.error('failed to start the plugin language server', e);
			scheduleTierProbe(id, tierProbeDelayMs);
			backOffTierProbe();
		}
	}

	async function probeTier(id: string): Promise<void> {
		if (destroyed) return;
		let status: TierStatus;
		try {
			status = await lspClient.probe();
		} catch (e) {
			if (destroyed) return;
			console.error('failed to probe the editor tier', e);
			scheduleTierProbe(id, tierProbeDelayMs);
			backOffTierProbe();
			return;
		}
		if (destroyed) return;
		if (status.tier === 'full') {
			await activateFullTier(id);
		} else if (status.tier === 'baseline') {
			tierProbeDelayMs = TIER_PROBE_INTERVAL_MS;
			toast.add(
				status.reason ?? 'The language server is unavailable; editing continues without it.',
				'Editing without full language support',
				'warning'
			);
		} else {
			scheduleTierProbe(id, TIER_PROBE_INTERVAL_MS);
		}
	}

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
			return;
		}
		if (destroyed) return;

		await probeTier(pluginId);
	});

	onDestroy(() => {
		destroyed = true;
		if (checkTimer) clearTimeout(checkTimer);
		if (tierProbeTimer) clearTimeout(tierProbeTimer);
		providers?.dispose();
		lspProviders.forEach((provider) => provider.dispose());
		lspClient.dispose();
		pluginEditor.reset();
	});

	function onSourceChanged(text: string) {
		pluginEditor.setSource(pluginEditor.activePath, text);
		if (checkTimer) clearTimeout(checkTimer);
		checkTimer = setTimeout(() => {
			pluginEditor.checkActive();
		}, CHECK_DEBOUNCE_MS);
	}

	interface LspLocationFrame {
		uri: string;
		range: LspRange;
	}

	interface LspTextEditFrame {
		range: LspRange;
		newText: string;
	}

	function isLspLocation(value: unknown): value is LspLocationFrame {
		return (
			typeof value === 'object' &&
			value !== null &&
			typeof (value as LspLocationFrame).uri === 'string' &&
			typeof (value as LspLocationFrame).range === 'object'
		);
	}

	function toMonacoLocations(
		raw: unknown,
		model: MonacoE.editor.ITextModel
	): MonacoE.languages.Location[] {
		const locations = Array.isArray(raw)
			? raw.filter(isLspLocation)
			: isLspLocation(raw)
				? [raw]
				: [];
		return locations
			.filter((location) => lspClient.pathFor(location.uri) === pluginEditor.activePath)
			.map((location) => ({ uri: model.uri, range: rangeFromLsp(location.range) }));
	}

	function toWorkspaceEdit(
		raw: unknown,
		model: MonacoE.editor.ITextModel
	): MonacoE.languages.WorkspaceEdit {
		const changes = (raw as { changes?: Record<string, LspTextEditFrame[]> } | null)?.changes ?? {};
		const edits: MonacoE.languages.IWorkspaceTextEdit[] = [];
		for (const [uri, textEdits] of Object.entries(changes)) {
			if (lspClient.pathFor(uri) !== pluginEditor.activePath) continue;
			for (const edit of textEdits) {
				edits.push({
					resource: model.uri,
					textEdit: { range: rangeFromLsp(edit.range), text: edit.newText },
					versionId: undefined
				});
			}
		}
		return { edits };
	}

	function onEditorReady(instance: MonacoE.editor.IStandaloneCodeEditor) {
		if (!monaco) return;
		providers?.dispose();
		providers = registerLuaProviders(monaco, () => snapshot, () => fullTierLive);

		const completionKind = monaco.languages.CompletionItemKind;
		const insertAsSnippet = monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet;
		const monacoCompletionKinds: Record<LspCompletionKind, MonacoE.languages.CompletionItemKind> = {
			text: completionKind.Text,
			method: completionKind.Method,
			function: completionKind.Function,
			constructor: completionKind.Constructor,
			field: completionKind.Field,
			variable: completionKind.Variable,
			class: completionKind.Class,
			interface: completionKind.Interface,
			module: completionKind.Module,
			property: completionKind.Property,
			enum: completionKind.Enum,
			keyword: completionKind.Keyword,
			snippet: completionKind.Snippet,
			enumMember: completionKind.EnumMember,
			constant: completionKind.Constant,
			struct: completionKind.Struct,
			event: completionKind.Event,
			operator: completionKind.Operator,
			typeParameter: completionKind.TypeParameter
		};

		lspProviders.forEach((provider) => provider.dispose());
		lspProviders = [
			monaco.languages.registerCompletionItemProvider('lua', {
				triggerCharacters: ['.', ':'],
				provideCompletionItems(model, position) {
					if (!fullTierLive) return { suggestions: [] };
					const word = model.getWordUntilPosition(position);
					const wordRange = {
						startLineNumber: position.lineNumber,
						endLineNumber: position.lineNumber,
						startColumn: word.startColumn,
						endColumn: word.endColumn
					};
					return lspClient
						.completion(pluginEditor.activePath, position.lineNumber, position.column)
						.then((raw) => ({
							suggestions: toCompletionSuggestions(raw).map((item) => ({
								label: item.label,
								kind: monacoCompletionKinds[item.kind],
								detail: item.detail,
								documentation: { value: item.documentation },
								insertText: item.insertText,
								insertTextRules: item.isSnippet ? insertAsSnippet : undefined,
								range: item.range ? rangeFromLsp(item.range) : wordRange
							}))
						}))
						.catch((e) => {
							onLspRequestFailed(e);
							return { suggestions: [] };
						});
				}
			}),
			monaco.languages.registerHoverProvider('lua', {
				provideHover(model, position) {
					if (!fullTierLive) return null;
					return lspClient
						.hover(pluginEditor.activePath, position.lineNumber, position.column)
						.then((raw) => toMonacoHover(raw))
						.catch((e) => {
							onLspRequestFailed(e);
							return null;
						});
				}
			}),
			monaco.languages.registerDefinitionProvider('lua', {
				provideDefinition(model, position) {
					if (!fullTierLive) return null;
					return lspClient
						.definition(pluginEditor.activePath, position.lineNumber, position.column)
						.then((raw) => toMonacoLocations(raw, model))
						.catch((e) => {
							onLspRequestFailed(e);
							return null;
						});
				}
			}),
			monaco.languages.registerReferenceProvider('lua', {
				provideReferences(model, position) {
					if (!fullTierLive) return [];
					return lspClient
						.references(pluginEditor.activePath, position.lineNumber, position.column)
						.then((raw) => toMonacoLocations(raw, model))
						.catch((e) => {
							onLspRequestFailed(e);
							return [];
						});
				}
			}),
			monaco.languages.registerRenameProvider('lua', {
				provideRenameEdits(model, position, newName) {
					if (!fullTierLive) return { edits: [] };
					return lspClient
						.rename(pluginEditor.activePath, position.lineNumber, position.column, newName)
						.then((raw) => toWorkspaceEdit(raw, model))
						.catch((e) => {
							onLspRequestFailed(e);
							return { edits: [] };
						});
				}
			})
		];

		instance.getModel()?.onDidChangeContent(() => {
			const text = instance.getValue();
			onSourceChanged(text);
			if (fullTierLive) lspClient.didChange(pluginEditor.activePath, text);
		});
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

	$effect(() => {
		const model = editor?.getModel();
		if (!monaco || !model) return;
		const severities: Record<DiagnosticSeverity, MonacoE.MarkerSeverity> = {
			error: monaco.MarkerSeverity.Error,
			warning: monaco.MarkerSeverity.Warning,
			info: monaco.MarkerSeverity.Info,
			hint: monaco.MarkerSeverity.Hint
		};
		const diagnostics = lspDiagnosticsByPath[pluginEditor.activePath] ?? [];
		const markers: MonacoE.editor.IMarkerData[] = diagnostics.map((diagnostic) => ({
			severity: severities[diagnostic.severity],
			message: diagnostic.message,
			...diagnostic.range
		}));
		monaco.editor.setModelMarkers(model, 'psp-lsp', markers);
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
