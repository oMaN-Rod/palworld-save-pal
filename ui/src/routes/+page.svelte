<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { goto } from '$app/navigation';
	import { getAppState } from '$states';
	import { Card, FileDropzone, Tooltip } from '$components/ui';
	import { GamepassBrowser } from '$components/gamepass';
	import { send, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { getStoredSessionId, clearSessionPersistence } from '$lib/utils/sessionPersistence';
	import { MessageType, type GamepassSave } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { Download, Upload, Save } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	const appState = getAppState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);

	let files: FileList | undefined = $state();

	async function handleSelectSave(t: string) {
		await goto('/loading');
		send(MessageType.SELECT_SAVE, { type: t, local: isDesktopMode });
	}

	async function handleSelectGamepassSave(save: GamepassSave) {
		await goto('/loading');
		send(MessageType.SELECT_GAMEPASS_SAVE, save.save_id);
	}

	async function handleOnUpload() {
		if (!files) return;
		await goto('/loading');
		appState.resetState();
		pushProgressMessage('Uploading zip file...');
		const reader = new FileReader();
		reader.onload = function () {
			send(MessageType.LOAD_ZIP_FILE, Array.from(new Uint8Array(reader.result as ArrayBuffer)));
		};
		reader.readAsArrayBuffer(files[0]);
	}

	$effect(() => {
		if (files?.length) handleOnUpload();
	});

	async function handleDownload() {
		send(MessageType.DOWNLOAD_SAVE_FILE);
		await goto('/loading');
		pushProgressMessage('Starting to cook...');
	}

	async function handleEject() {
		const sessionId = getStoredSessionId();
		if (sessionId) send(MessageType.EJECT_SESSION, { session_id: sessionId });
		appState.resetState();
		clearSessionPersistence();
	}
</script>

<div class="flex min-h-full items-center justify-center p-4">
	<div class="flex w-full max-w-md flex-col items-center gap-4 text-center">
		<h1 class="heading-gradient text-xl font-extrabold tracking-tight">
			{isDesktopMode ? 'Load Save' : 'Upload Save'}
		</h1>

		{#if !appState.saveFile}
			{#if isDesktopMode}
				<div class="flex w-full flex-col gap-3">
					<button class="btn btn-primary w-full" onclick={() => handleSelectSave('steam')}>
						<span class="inline-block h-5 w-5">{@html steamIcon}</span>
						{m.steam()}
					</button>
					<button class="btn btn-secondary w-full" onclick={() => handleSelectSave('gamepass')}>
						<span class="inline-block h-5 w-5">{@html xboxIcon}</span>
						Xbox Game Pass
					</button>
					{#if appState.gamepassSaves && Object.keys(appState.gamepassSaves).length > 0}
						<Card class="mt-2 w-full">
							<GamepassBrowser
								saves={appState.gamepassSaves}
								selectable={true}
								onselect={handleSelectGamepassSave}
							/>
						</Card>
					{/if}
				</div>
			{:else}
				<Card class="w-full">
					<FileDropzone baseClass="w-full hover:bg-surface-800" name="file" bind:files>
						{#snippet message()}
							<div class="flex flex-col items-center gap-2 py-4">
								<Upload class="h-8 w-8 text-muted" />
								<p class="text-sm font-semibold">{m.upload_zip_files()}</p>
								<p class="text-xs text-muted">{m.drag_drop_zip()}</p>
							</div>
						{/snippet}
					</FileDropzone>
				</Card>
			{/if}
		{:else}
			<Card class="flex w-full flex-col items-center gap-3 py-4">
				<Save class="h-8 w-8 text-success-400" />
				<p class="text-sm font-semibold text-surface-50">{appState.saveFile.world_name}</p>
				<p class="text-xs text-muted">{appState.saveFile.name}</p>
				<div class="flex gap-2">
					{#if !isDesktopMode}
						<Tooltip>
							{#snippet children()}
								<button class="btn btn-primary" onclick={handleDownload}>
									<Download class="h-4 w-4" />
									{m.download()}
								</button>
							{/snippet}
							{#snippet popup()}
								<span>{m.download_modified_save()}</span>
							{/snippet}
						</Tooltip>
					{/if}
					<button class="btn btn-ghost" onclick={handleEject}>
						Unload
					</button>
				</div>
			</Card>
		{/if}
	</div>
</div>
