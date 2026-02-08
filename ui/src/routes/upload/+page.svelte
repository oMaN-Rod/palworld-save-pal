<script lang="ts">
	import { FileDropzone, Card, Tooltip } from '$components/ui';
	import { MessageType } from '$types';
	import { getAppState } from '$states';
	import { Download } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let appState = getAppState();

	let files: FileList | undefined = $state();

	async function handleOnUpload() {
		if (!files) return;
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Uploading zip file 🚀...');
		const reader = new FileReader();
		reader.onload = function () {
			const arrayBuffer = reader.result as ArrayBuffer;
			const uint8Array = new Uint8Array(arrayBuffer);
			send(MessageType.LOAD_ZIP_FILE, Array.from(uint8Array));
		};
		reader.readAsArrayBuffer(files[0]);
	}

	async function handleDownloadSaveFile() {
		send(MessageType.DOWNLOAD_SAVE_FILE);
		await goto('/loading');
		pushProgressMessage('Starting to cook 🧑‍🍳...');
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center space-y-4">
	{#if appState.saveFile}
		<Card class="w-1/3">
			<div class="flex">
				<div class="flex grow flex-col">
					<h4 class="h4">{m.current_save_file()}</h4>
					<p class="text"><strong>{m.file({ count: 1 })}</strong> {appState.saveFile.name}</p>
					<p class="text">
						<strong>{m.size()}</strong>
						{(appState.saveFile.size! / 1024 / 1024).toFixed(2)} MB
					</p>
				</div>
				<div class="flex flex-col space-y-2">
					<Tooltip>
						<button
							class="btn preset-filled-primary-500 font-bold"
							onclick={handleDownloadSaveFile}
						>
							<Download />
							{m.download()}
						</button>
						{#snippet popup()}
							<span>{m.download_modified_save()}</span>
						{/snippet}
					</Tooltip>
				</div>
			</div>
		</Card>
	{/if}
	<div class="flex w-1/3 flex-row justify-center">
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
							<button class="btn preset-filled-primary-500 font-bold" onclick={handleOnUpload}>
								{m.upload()}
							</button>
						{/snippet}
						{#snippet popup()}
							<span>{m.upload()} {files ? files[0].name : ''}</span>
						{/snippet}
					</Tooltip>
				</div>
			{/if}
		</div>
	</div>
</div>
