<script lang="ts">
	import { FileDropzone, Card, Tooltip } from '$components/ui';
	import { getSocketState } from '$states/websocketState.svelte';
	import { MessageType } from '$types';
	import { getAppState, getNavigationState, getToastState } from '$states';

	let appState = getAppState();
	const ws = getSocketState();
	const nav = getNavigationState();
	const toast = getToastState();

	let files: FileList | undefined = $state();

	function handleOnUpload() {
		if (!files) return;
		nav.activePage = 'Loading';
		ws.message = { type: MessageType.PROGRESS_MESSAGE, data: 'Uploading save file 🚀...' };
		const reader = new FileReader();
		reader.onload = function () {
			const arrayBuffer = reader.result as ArrayBuffer;
			const uint8Array = new Uint8Array(arrayBuffer);
			const data = {
				type: MessageType.LOAD_SAVE_FILE,
				data: Array.from(uint8Array)
			};

			ws.send(JSON.stringify(data));
		};
		reader.readAsArrayBuffer(files[0]);
	}

	function handleDownloadSaveFile() {
		ws.send(JSON.stringify({ type: MessageType.DOWNLOAD_SAVE_FILE }));
		toast.add('Generating sav file, grab a ☕...');
		nav.activePage = 'Loading';
		ws.message = { type: MessageType.PROGRESS_MESSAGE, data: 'Starting to cook 🧑‍🍳...' };
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center space-y-4">
	{#if appState.saveFile}
		<Card class="w-1/3">
			<div class="flex">
				<div class="flex grow flex-col">
					<h4 class="h4">Current Save File</h4>
					<p class="text"><strong>File:</strong> {appState.saveFile.name}</p>
					<p class="text">
						<strong>Size:</strong>
						{(appState.saveFile.size / 1024 / 1024).toFixed(2)} MB
					</p>
				</div>
				<button class="btn preset-filled-primary-500 font-bold" onclick={handleDownloadSaveFile}>
					Download
				</button>
			</div>
		</Card>
	{/if}
	<div class="flex w-1/3 flex-row justify-center">
		<div class="flex w-full flex-col items-center">
			<FileDropzone baseClass="w-full hover:bg-surface-800" name="file" bind:files>
				{#snippet message()}
					<h3 class="h3">Click to upload a save file</h3>
					<span>or drag and drop it here</span>
				{/snippet}
			</FileDropzone>
			{#if files}
				<div class="mt-2 flex flex-col">
					<Tooltip>
						{#snippet children()}
							<button class="btn preset-filled-primary-500 font-bold" onclick={handleOnUpload}>
								Upload
							</button>
						{/snippet}
						{#snippet popup()}
							<span>Upload {files ? files[0].name : ''}</span>
						{/snippet}
					</Tooltip>
				</div>
			{/if}
		</div>
	</div>
</div>
