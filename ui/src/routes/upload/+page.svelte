<script lang="ts">
	import { Card, Tooltip, Button } from '$components/ui';
	import { SaveDropzone } from '$components/upload';
	import { MessageType } from '$types';
	import { getAppState, getToastState } from '$states';
	import Download from '@lucide/svelte/icons/download';
	import Settings2 from '@lucide/svelte/icons/settings-2';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import { goto } from '$app/navigation';
	import { send, sendBytes, pushProgressMessage } from '$lib/utils/websocketUtils';
	import { startSaveLoad } from '$lib/data/loadSave';
	import { openWorldOptionModal } from '$components/worldoption';
	import {
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

	let recentName = $state<string | null>(null);

	$effect(() => {
		if (isWebBuild) hasRecent().then((r) => (recentName = r?.worldName ?? null));
	});

	async function resume() {
		await goto('/loading');
		appState.resetState();
		pushProgressMessage(m.upload_restoring());
		const r = await restoreMostRecent((bytes) => sendBytes(MessageType.LOAD_ZIP_FILE, bytes));
		if (!r.restored) {
			await goto('/upload');
			toast.add(
				r.needsPermission ? m.upload_reconnect_folder() : m.upload_restore_failed(),
				m.toast_heads_up(),
				'warning'
			);
		}
	}

	function saveToFolder() {
		setSaveTarget('folder');
		send(MessageType.DOWNLOAD_SAVE_FILE);
	}

	async function handleDownloadSaveFile() {
		send(MessageType.DOWNLOAD_SAVE_FILE);
		await goto('/loading');
		pushProgressMessage(m.upload_starting_to_cook());
	}
</script>

<div class="animate-fade-in flex h-full w-full flex-col items-center justify-center space-y-4">
	{#if recentName && !appState.saveFile}
		<Button variant="secondary" onclick={resume}>
			<FolderOpen size={16} />
			{m.upload_resume({ name: recentName })}
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
					{#if isWebBuild && getActiveDirectory().writable}
						<Button variant="secondary" onclick={saveToFolder}>
							<FolderOpen size={16} />
							{m.upload_save_to_folder()}
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
	<div class="flex w-full max-w-xl flex-col items-center px-4 sm:w-3/4 md:w-1/2 lg:w-1/3">
		<SaveDropzone onLoad={startSaveLoad} />
		<p class="mt-2 max-w-md text-center text-xs opacity-60">
			{m.upload_path_hint()}
		</p>
	</div>
</div>
