<script module lang="ts">
	export const exportedThemes = Object.fromEntries(
		Object.entries(import.meta.glob('/node_modules/monaco-themes/themes/*.json')).map(([k, v]) => [
			k.toLowerCase().split('/').reverse()[0].slice(0, -'.json'.length).replaceAll(' ', '-'),
			v
		])
	);

	export const nativeThemes = ['vs', 'vs-dark', 'hc-black'];

	export const themeNames: string[] = [...Object.keys(exportedThemes), ...nativeThemes].sort(
		(a, b) => a.localeCompare(b)
	);
</script>

<script lang="ts">
	import type MonacoE from 'monaco-editor';
	import { onDestroy, onMount } from 'svelte';
	import loader from '@monaco-editor/loader';

	let container: HTMLDivElement | undefined = $state();

	interface Props {
		editor?: MonacoE.editor.IStandaloneCodeEditor | undefined;
		value: string;
		language?: string | undefined;
		theme?: string | undefined;
		options?: MonacoE.editor.IStandaloneEditorConstructionOptions;
		largeFile?: boolean;
		monaco?: typeof MonacoE;
		onready?: (editor: MonacoE.editor.IStandaloneCodeEditor) => void;
	}

	const largeFileOptions: MonacoE.editor.IStandaloneEditorConstructionOptions = {
		largeFileOptimizations: true,
		maxTokenizationLineLength: 5000,
		stopRenderingLineAfter: -1,
		folding: false,
		minimap: { enabled: false },
		renderWhitespace: 'none',
		renderLineHighlight: 'none',
		wordWrap: 'on'
	};

	let {
		editor = $bindable(undefined),
		value = $bindable(),
		language = undefined,
		theme = undefined,
		options = {
			value,
			automaticLayout: true
		},
		largeFile = false,
		monaco = $bindable(),
		onready = undefined
	}: Props = $props();

	let mergedOptions = $derived({
		...options,
		...(language ? { language } : {}),
		...(largeFile ? largeFileOptions : {})
	});

	function refreshTheme() {
		if (theme) {
			if (exportedThemes[theme]) {
				const themeName = theme; // the theme name can change during the async call
				exportedThemes[theme]().then((resolvedTheme) => {
					monaco?.editor.defineTheme(themeName, resolvedTheme as any);
					monaco?.editor.setTheme(themeName);
				});
			} else if (nativeThemes.includes(theme)) {
				monaco?.editor.setTheme(theme);
			}
		}
	}

	$effect(() => {
		if (theme) refreshTheme();
	});

	$effect(() => {
		editor?.updateOptions(mergedOptions);
	});

	let model = $derived(editor?.getModel());

	$effect(() => {
		model && mergedOptions.language
			? monaco!.editor.setModelLanguage(model, mergedOptions.language)
			: void 0;
	});

	$effect(() => {
		if (editor && editor.getValue() != value) {
			const position = editor.getPosition();
			editor.setValue(value);
			if (position) editor.setPosition(position);
		}
	});

	onMount(async () => {
		monaco = await loader.init();
		editor = monaco!.editor.create(container!, mergedOptions);

		onready?.(editor);

		refreshTheme();

		editor.getModel()!.onDidChangeContent(() => {
			if (!editor) return;
			value = editor.getValue();
		});
	});

	onDestroy(() => editor?.dispose());
</script>

<div class="monaco-container" bind:this={container}></div>

<style>
	div.monaco-container {
		width: 100%;
		height: 100%;
		padding: 0;
		margin: 0;
	}
</style>
