<script lang="ts">
	import { FileDropzone, Card, Tooltip, Button } from '$components/ui';
	import { MessageType } from '$types';
	import { getAppState, getToastState } from '$states';
	import { Download, Settings2, FolderOpen } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { startSaveLoad } from '$lib/data/loadSave';
	import { openWorldOptionModal } from '$components/worldoption';
	import {
		readInputFolder,
		readDroppedItems,
		zipEntries,
		hasLevelSav,
		type ZipEntry
	} from '$lib/utils/folderUpload';
	import {
		fsaSupported,
		pickSaveDirectory,
		readSaveFolder,
		ensureReadWrite,
		restoreMostRecent,
		hasRecent,
		setSaveTarget,
		getActiveDirectory
	} from '$lib/fs';
	import { isWebBuild } from '$lib/utils/platform';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let appState = getAppState();
	let toast = getToastState();

	let files: FileList | undefined = $state();
	let folderInput: HTMLInputElement | undefined = $state();
	let folderDragOver = $state(false);
	let folderError = $state('');

	const canUseFsa = isWebBuild && fsaSupported();
	let recentName = $state<string | null>(null);

	$effect(() => {
		if (isWebBuild) hasRecent().then((r) => (recentName = r?.worldName ?? null));
	});

	// Set the non-standard directory-picker flags via the property; some browsers
	// ignore the bare attribute.
	$effect(() => {
		if (folderInput) {
			folderInput.webkitdirectory = true;
			(folderInput as HTMLInputElement & { directory?: boolean }).directory = true;
		}
	});

	async function startLoad(
		zip: Uint8Array,
		name: string,
		source?: { handle?: FileSystemDirectoryHandle; writable?: boolean }
	) {
		await startSaveLoad(zip, name, source);
	}

	async function handleOnUpload() {
		if (!files) return;
		const file = files[0];
		const reader = new FileReader();
		reader.onload = async function () {
			const arrayBuffer = reader.result as ArrayBuffer;
			await startLoad(new Uint8Array(arrayBuffer), file.name);
		};
		reader.readAsArrayBuffer(file);
	}

	async function loadEntries(entries: ZipEntry[]) {
		if (!hasLevelSav(entries)) {
			folderError =
				'That folder has no Level.sav — choose the world save folder itself (the one containing Level.sav and Players/).';
			return;
		}
		folderError = '';
		await startLoad(zipEntries(entries), 'save');
	}

	async function openWithPicker() {
		const dir = await pickSaveDirectory();
		if (!dir) return;
		let entries;
		try {
			entries = await readSaveFolder(dir);
		} catch (e) {
			folderError = e instanceof Error ? e.message : String(e);
			return;
		}
		const writable = await ensureReadWrite(dir);
		await startLoad(zipEntries(entries), dir.name, { handle: dir, writable });
	}

	async function resume() {
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Restoring your last save...');
		const r = await restoreMostRecent((bytes) => send(MessageType.LOAD_ZIP_FILE, Array.from(bytes)));
		if (!r.restored) {
			await goto('/upload');
			toast.add(
				r.needsPermission
					? 'Click "Open save folder" to reconnect your save folder.'
					: 'Could not restore the last save.',
				'Heads up',
				'warning'
			);
		}
	}

	function saveToFolder() {
		setSaveTarget('folder');
		send(MessageType.DOWNLOAD_SAVE_FILE);
	}

	async function onFolderChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		if (!input.files || input.files.length === 0) return;
		await loadEntries(await readInputFolder(input.files));
	}

	async function onFolderDrop(event: DragEvent) {
		event.preventDefault();
		folderDragOver = false;
		if (!event.dataTransfer) return;
		const entries = await readDroppedItems(event.dataTransfer.items);
		if (entries.length === 0) {
			folderError =
				'Could not read that drop — try the "Choose folder" button, or drop the save folder itself.';
			return;
		}
		await loadEntries(entries);
	}

	async function handleDownloadSaveFile() {
		send(MessageType.DOWNLOAD_SAVE_FILE);
		await goto('/loading');
		pushProgressMessage('Starting to cook...');
	}
</script>

<div class="animate-fade-in flex h-full w-full flex-col items-center justify-center space-y-4">
	{#if recentName && !appState.saveFile}
		<Button variant="secondary" onclick={resume}>
			<FolderOpen size={16} />
			Resume {recentName}
		</Button>
	{/if}
	{#if appState.saveFile}
		<Card class="w-full max-w-xl px-4 sm:w-3/4 md:w-1/2 lg:w-1/3">
			<div class="flex">
				<div class="flex grow flex-col">
					<h4 class="h4">{m.current_save_file()}</h4>
					<p class="text"><strong>{m.file({ count: 1 })}</strong> {appState.saveFile.name}</p>
					{#if typeof appState.saveFile.size === 'number' && !isNaN(appState.saveFile.size)}
						<p class="text">
							<strong>{m.size()}</strong>
							{(appState.saveFile.size / 1024 / 1024).toFixed(2)} MB
						</p>
					{/if}
				</div>
				<div class="flex flex-col space-y-2">
					<Tooltip>
						<Button variant="primary" class="font-bold" onclick={handleDownloadSaveFile}>
							<Download />
							{m.download()}
						</Button>
						{#snippet popup()}
							<span>{m.download_modified_save()}</span>
						{/snippet}
					</Tooltip>
					{#if canUseFsa && getActiveDirectory().writable}
						<Button variant="secondary" onclick={saveToFolder}>
							<FolderOpen size={16} />
							Save to folder
						</Button>
					{/if}
					{#if appState.saveFile.world_option_present}
						<Button variant="secondary" onclick={openWorldOptionModal}>
							<Settings2 size={16} />
							World Options
						</Button>
					{/if}
				</div>
			</div>
		</Card>
	{/if}
	<div class="flex w-full max-w-xl flex-row justify-center px-4 sm:w-3/4 md:w-1/2 lg:w-1/3">
		<div class="flex w-full flex-col items-center">
			<FileDropzone baseClass="w-full hover:bg-surface-800" name="file" bind:files>
				{#snippet message()}
					<h3 class="h3">{m.upload_zip_files()}</h3>
					<span>{m.drag_drop_zip()}</span>
				{/snippet}
			</FileDropzone>
			{#if files}
				<div class="mt-2 flex flex-col">
					<Tooltip>
						{#snippet children()}
							<Button variant="primary" class="font-bold" onclick={handleOnUpload}>
								{m.upload()}
							</Button>
						{/snippet}
						{#snippet popup()}
							<span>{m.upload()} {files ? files[0].name : ''}</span>
						{/snippet}
					</Tooltip>
				</div>
			{/if}

			<div class="mt-4 flex w-full items-center gap-2 opacity-60">
				<div class="bg-surface-500 h-px flex-1"></div>
				<span class="text-sm">or</span>
				<div class="bg-surface-500 h-px flex-1"></div>
			</div>

			{#if canUseFsa}
				<Button variant="secondary" class="mt-4" onclick={openWithPicker}>
					<FolderOpen size={16} />
					Open save folder
				</Button>
			{:else}
				<div
					role="button"
					tabindex="0"
					class="textarea rounded-container-token relative mt-4 flex w-full flex-col items-center justify-center border-2 border-dashed p-4 py-8 {folderDragOver
						? 'bg-surface-800'
						: 'hover:bg-surface-800'}"
					ondragover={(e) => {
						e.preventDefault();
						folderDragOver = true;
					}}
					ondragleave={() => (folderDragOver = false)}
					ondrop={onFolderDrop}
					onclick={() => folderInput?.click()}
					onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && folderInput?.click()}
				>
					<FolderOpen class="h-16 w-16" />
					<h3 class="h3 mt-2">Drop a save folder</h3>
					<span>Drag your world folder here (Level.sav, Players/, …), or</span>
					<Button
						variant="secondary"
						class="mt-2"
						onclick={(e: MouseEvent) => {
							e.stopPropagation();
							folderInput?.click();
						}}
					>
						Choose folder
					</Button>
				</div>
				<input bind:this={folderInput} type="file" multiple class="hidden" onchange={onFolderChange} />
			{/if}
			<p class="mt-2 max-w-md text-center text-xs opacity-60">
				Pick the world save folder that contains Level.sav and a Players/ folder. On Steam this is usually
				…/Steam/steamapps/common/Palworld/Pal/Saved/SaveGames/&lt;id&gt;/&lt;world&gt;.
			</p>
			{#if folderError}
				<p class="text-error-400 mt-2 text-sm">{folderError}</p>
			{/if}
		</div>
	</div>
</div>
