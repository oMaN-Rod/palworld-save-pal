<script lang="ts">
	import { FileDropzone, Card, Tooltip, Button } from '$components/ui';
	import { MessageType } from '$types';
	import { getAppState } from '$states';
	import { Download, Settings2, FolderOpen } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { openWorldOptionModal } from '$components/worldoption';
	import {
		readInputFolder,
		readDroppedItems,
		zipEntries,
		hasLevelSav,
		type ZipEntry
	} from '$lib/utils/folderUpload';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let appState = getAppState();

	let files: FileList | undefined = $state();
	let folderInput: HTMLInputElement | undefined = $state();
	let folderDragOver = $state(false);
	let folderError = $state('');

	// Set the non-standard directory-picker flags via the property; some browsers
	// ignore the bare attribute.
	$effect(() => {
		if (folderInput) {
			folderInput.webkitdirectory = true;
			(folderInput as HTMLInputElement & { directory?: boolean }).directory = true;
		}
	});

	async function handleOnUpload() {
		if (!files) return;
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Uploading zip file...');
		const reader = new FileReader();
		reader.onload = function () {
			const arrayBuffer = reader.result as ArrayBuffer;
			const uint8Array = new Uint8Array(arrayBuffer);
			send(MessageType.LOAD_ZIP_FILE, Array.from(uint8Array));
		};
		reader.readAsArrayBuffer(files[0]);
	}

	async function loadEntries(entries: ZipEntry[]) {
		if (!hasLevelSav(entries)) {
			folderError =
				'That folder has no Level.sav — choose the world save folder itself (the one containing Level.sav and Players/).';
			return;
		}
		folderError = '';
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Reading save folder...');
		const zip = zipEntries(entries);
		send(MessageType.LOAD_ZIP_FILE, Array.from(zip));
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
			{#if folderError}
				<p class="text-error-400 mt-2 text-sm">{folderError}</p>
			{/if}
		</div>
	</div>
</div>
