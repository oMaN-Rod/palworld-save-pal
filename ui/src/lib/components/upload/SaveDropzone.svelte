<script lang="ts">
	import { Button } from '$components/ui';
	import FolderArchive from '@lucide/svelte/icons/folder-archive';
	import FileArchive from '@lucide/svelte/icons/file-archive';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import {
		readInputFolder,
		readDroppedItems,
		zipEntries,
		hasLevelSav,
		hasDirectoryEntry,
		type ZipEntry
	} from '$lib/utils/folderUpload';
	import { fsaSupported, pickSaveDirectory, readSaveFolder, ensureReadWrite } from '$lib/fs';
	import { isWebBuild } from '$lib/utils/platform';
	import * as m from '$i18n/messages';

	let {
		onLoad
	}: {
		onLoad: (
			zip: Uint8Array,
			name: string,
			source?: { handle?: FileSystemDirectoryHandle; writable?: boolean }
		) => void;
	} = $props();

	let zipInput: HTMLInputElement | undefined = $state();
	let folderInput: HTMLInputElement | undefined = $state();
	let dragOver = $state(false);
	let error = $state('');

	// webkitdirectory must be set via property; some browsers ignore the attribute.
	$effect(() => {
		if (folderInput) {
			folderInput.webkitdirectory = true;
			(folderInput as HTMLInputElement & { directory?: boolean }).directory = true;
		}
	});

	function loadEntries(entries: ZipEntry[]) {
		if (!hasLevelSav(entries)) {
			error = m.upload_error_no_level_sav();
			return;
		}
		error = '';
		onLoad(zipEntries(entries), 'save');
	}

	async function loadZipFile(file: File) {
		error = '';
		onLoad(new Uint8Array(await file.arrayBuffer()), file.name);
	}

	async function onDrop(event: DragEvent) {
		event.preventDefault();
		dragOver = false;
		const dt = event.dataTransfer;
		if (!dt) return;
		if (hasDirectoryEntry(dt.items)) {
			const entries = await readDroppedItems(dt.items);
			if (entries.length === 0) {
				error = m.upload_error_folder_read();
				return;
			}
			loadEntries(entries);
			return;
		}
		const file = Array.from(dt.files).find((f) => f.name.toLowerCase().endsWith('.zip'));
		if (file) {
			await loadZipFile(file);
			return;
		}
		error = m.upload_error_drop_type();
	}

	async function onZipChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		if (input.files?.length) await loadZipFile(input.files[0]);
	}
	async function onFolderChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		if (input.files?.length) loadEntries(await readInputFolder(input.files));
	}

	// Prefer the File System Access picker (yields a handle → enables save-in-place);
	// fall back to a webkitdirectory input where FSA is unavailable.
	async function chooseFolder() {
		if (isWebBuild && fsaSupported()) {
			const dir = await pickSaveDirectory();
			if (!dir) return;
			let entries: ZipEntry[];
			try {
				entries = await readSaveFolder(dir);
			} catch (e) {
				error = e instanceof Error ? e.message : String(e);
				return;
			}
			error = '';
			const writable = await ensureReadWrite(dir);
			onLoad(zipEntries(entries), dir.name, { handle: dir, writable });
			return;
		}
		folderInput?.click();
	}
</script>

<div class="flex w-full flex-col items-center">
	<div
		role="button"
		tabindex="0"
		class="textarea rounded-container-token relative flex w-full flex-col items-center justify-center border-2 border-dashed p-6 py-10 {dragOver
			? 'bg-surface-800'
			: 'hover:bg-surface-800'}"
		ondragover={(e) => {
			e.preventDefault();
			dragOver = true;
		}}
		ondragleave={() => (dragOver = false)}
		ondrop={onDrop}
	>
		<FolderArchive class="h-16 w-16 opacity-80" />
		<h3 class="h3 mt-3">{m.upload_drop_heading()}</h3>
		<span class="text-surface-300">{m.upload_drop_hint()}</span>
		<div class="mt-4 flex flex-wrap items-center justify-center gap-3">
			<Button variant="secondary" onclick={() => zipInput?.click()}>
				<FileArchive size={16} />
				{m.upload_choose_zip()}
			</Button>
			<Button variant="secondary" onclick={chooseFolder}>
				<FolderOpen size={16} />
				{m.upload_choose_folder()}
			</Button>
		</div>
	</div>
	<input bind:this={zipInput} type="file" accept=".zip" class="hidden" onchange={onZipChange} />
	<input bind:this={folderInput} type="file" multiple class="hidden" onchange={onFolderChange} />
	{#if error}
		<p class="text-error-400 mt-3 text-sm">{error}</p>
	{/if}
</div>
