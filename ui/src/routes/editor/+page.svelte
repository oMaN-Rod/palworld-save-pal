<script lang="ts">
	import { Stopwatch } from '$components';
	import Spinner from '$components/spinner/Spinner.svelte';
	import { FileDropzone } from '$components/ui';
	import Monaco from '$components/ui/monaco/Monaco.svelte';
	import { getToastState } from '$states';
	import { Save, X, WrapText } from 'lucide-svelte';

	const LARGE_FILE_THRESHOLD = 50 * 1024 * 1024; // 50MB

	let isLoading = $state(false);
	let content: { text: string } | undefined = $state(undefined);
	let formatted = $state(true);
	let largeFile = $state(false);
	let files: FileList | undefined = $state();
	let fileName: string | undefined = $state();
	let elapsed = $state(0);
	let intervalId: ReturnType<typeof setInterval> | null = null;

	const toast = getToastState();

	async function handleSave() {
		if (content?.text) {
			try {
				const response = await fetch('/api/convert/json-to-sav', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: content.text
				});
				if (!response.ok) throw new Error(`Server error: ${response.status}`);
				const blob = await response.blob();
				const url = URL.createObjectURL(blob);
				const a = document.createElement('a');
				a.href = url;
				a.download = fileName ? fileName : 'modified_save.sav';
				document.body.appendChild(a);
				a.click();
				document.body.removeChild(a);
				URL.revokeObjectURL(url);
			} catch (error) {
				console.error('Error saving file:', error);
				toast.add('Failed to save file.');
			}
		} else {
			toast.add('Nothing to save.');
		}
	}

	function handleClose() {
		content = undefined;
		files = undefined;
	}

	function toggleFormatting() {
		if (!content?.text) return;
		try {
			if (formatted) {
				content = { text: JSON.stringify(JSON.parse(content.text)) };
			} else {
				content = { text: JSON.stringify(JSON.parse(content.text), null, 2) };
			}
			formatted = !formatted;
		} catch {
			toast.add('Could not toggle formatting — invalid JSON.');
		}
	}

	$effect(() => {
		if (files && files.length > 0) {
			isLoading = true;
			intervalId = setInterval(() => {
				elapsed += 1;
			}, 1000);
			const file = files[0];
			fileName = file.name;
			const reader = new FileReader();
			reader.onload = async function () {
				try {
					const arrayBuffer = reader.result as ArrayBuffer;
					const formData = new FormData();
					formData.append('file', new Blob([arrayBuffer]), file.name);
					const response = await fetch('/api/convert/sav-to-json', {
						method: 'POST',
						body: formData
					});
					if (!response.ok) throw new Error(`Server error: ${response.status}`);
					const json = await response.text();
					const prettyJson = JSON.stringify(JSON.parse(json), null, 2);
					largeFile = prettyJson.length > LARGE_FILE_THRESHOLD;
					formatted = true;
					content = { text: prettyJson };
				} catch (error) {
					console.error('Error converting file:', error);
					content = undefined;
				}
				isLoading = false;
				if (intervalId) {
					clearInterval(intervalId);
					intervalId = null;
				}
			};
			reader.readAsArrayBuffer(file);
		} else {
			content = undefined;
		}
	});
</script>

{#if content}
	<div class="editor-wrapper">
		<div class="toolbar">
			<button class="toolbar-btn" title="Save SAV file" onclick={handleSave}>
				<Save size={18} />
				<span>Save</span>
			</button>
			<button
				class="toolbar-btn"
				class:active={formatted}
				title={formatted ? 'Minify JSON' : 'Format JSON'}
				onclick={toggleFormatting}
			>
				<WrapText size={18} />
				<span>Format</span>
			</button>
			<button class="toolbar-btn" title="Close file" onclick={handleClose}>
				<X size={18} />
				<span>Close</span>
			</button>
		</div>
		<div class="editor-container">
			<Monaco language="json" bind:value={content.text} theme="vs-dark" {largeFile} />
		</div>
	</div>
{:else}
	<div class="flex h-full w-full flex-col items-center justify-center">
		{#if isLoading}
			<Spinner />
			<Stopwatch bind:seconds={elapsed} />
		{:else}
			<FileDropzone baseClass="w-1/2 hover:bg-surface-800" name="file" bind:files>
				{#snippet message()}
					<h3 class="h3">Edit SAV</h3>
					<span>Drag and drop a *.sav file here</span>
				{/snippet}
			</FileDropzone>
		{/if}
	</div>
{/if}

<style>
	.editor-wrapper {
		height: 100%;
		width: 100%;
		display: flex;
		flex-direction: column;
	}

	.toolbar {
		display: flex;
		gap: 0.5rem;
		padding: 0.5rem;
		background-color: rgb(var(--color-surface-800));
		border-bottom: 1px solid rgb(var(--color-surface-600));
	}

	.toolbar-btn {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		padding: 0.25rem 0.75rem;
		border-radius: 0.25rem;
		font-size: 0.875rem;
		color: rgb(var(--color-surface-100));
		background-color: rgb(var(--color-surface-700));
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.toolbar-btn:hover {
		background-color: rgb(var(--color-surface-600));
	}

	.toolbar-btn.active {
		background-color: rgb(var(--color-surface-600));
		outline: 1px solid rgb(var(--color-surface-400));
	}

	.editor-container {
		flex: 1;
		min-height: 0;
	}
</style>
