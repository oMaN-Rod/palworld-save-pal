<script lang="ts">
	import { getNavigationState, getSocketState, getAppState, getToastState } from '$states';
	import Edit from '$lib/pages/Edit.svelte';
	import Info from '$lib/pages/Info.svelte';
	import Settings from '$lib/pages/Settings.svelte';
	import File from '$lib/pages/File.svelte';
	import { Spinner } from '$components';
	import { MessageType, type Message } from '$types';
	import { palsData } from '$lib/data';

	const appState = getAppState();
	const ws = getSocketState();
	const nav = getNavigationState();
	const toast = getToastState();

	let progressMessage = $state('');

	$effect(() => {
		if (ws.message && ws.message.type) {
			const { data, type } = ws.message;
			switch (type) {
				case MessageType.GET_PLAYERS:
					appState.players = data;
					ws.clear(MessageType.GET_PLAYERS);
					const numOfPlayers = Object.keys(data).length;
					toast.add(`${numOfPlayers} players loaded successfully`, 'Success!');
					break;
				case MessageType.LOAD_SAVE_FILE:
					const file = data as { name: string; size: number };
					appState.saveFile = file;
					ws.clear(MessageType.LOAD_SAVE_FILE);
					nav.activePage = 'Edit';
					toast.add(`Save file uploaded successfully as ${file.name}`, 'Success!');
					break;
				case MessageType.DOWNLOAD_SAVE_FILE:
					console.log('Download save file', data);
					const { name, content } = data as { name: string; content: string };

					// Decode the base64 string
					const binaryString = atob(content);
					const len = binaryString.length;
					const bytes = new Uint8Array(len);
					for (let i = 0; i < len; i++) {
						bytes[i] = binaryString.charCodeAt(i);
					}

					// Create blob from the decoded data
					const blob = new Blob([bytes], { type: 'application/octet-stream' });
					const url = URL.createObjectURL(blob);
					const a = document.createElement('a');
					a.href = url;
					a.download = name;
					a.click();
					URL.revokeObjectURL(url);
					ws.clear(MessageType.DOWNLOAD_SAVE_FILE);
					nav.activePage = 'File';
					break;
				case MessageType.SYNC_APP_STATE:
				case MessageType.GET_PAL_DETAILS:
					const checkMessage = async (message: Message) => {
						const { data } = message;
						let newPal = JSON.parse(data);
						const palInfo = await palsData.getPalInfo(newPal.character_id);
						newPal.name = palInfo?.localized_name || 'Unknown';
						newPal.elements = palInfo?.elements || [];
						appState.selectedPal = newPal;
						ws.clear(MessageType.GET_PAL_DETAILS);
					};
					checkMessage(ws.message);
					break;
				case MessageType.ERROR:
					const errorMessage = data as string;
					toast.add(errorMessage, 'Error', 'error');
					ws.clear(MessageType.ERROR);
					break;
				case MessageType.PROGRESS_MESSAGE:
					progressMessage = data as string;
					ws.clear(MessageType.PROGRESS_MESSAGE);
					break;
				case MessageType.UPDATE_SAVE_FILE:
					const updateMessage = data as string;
					ws.clear(MessageType.UPDATE_SAVE_FILE);
					toast.add(updateMessage, 'Success!');
					nav.activePage = 'Edit';
					break;
			}
		}
	});
</script>

<div class="flex h-full w-full">
	{#if nav.activePage === 'Edit'}
		<Edit />
	{:else if nav.activePage === 'File'}
		<File />
	{:else if nav.activePage === 'Info'}
		<Info />
	{:else if nav.activePage === 'Settings'}
		<Settings />
	{:else if nav.activePage === 'Loading'}
		<div class="flex h-full w-full flex-col items-center justify-center">
			<h2 class="h2 mb-8">🤖 Beep Boop, working on it!</h2>
			<Spinner size="size-32" />
			{#if progressMessage}
				<span class="mt-2">{progressMessage}</span>
			{/if}
		</div>
	{/if}
</div>
