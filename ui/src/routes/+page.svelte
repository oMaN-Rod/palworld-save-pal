<script lang="ts">
	import { getNavigationState, getSocketState, getAppState, getToastState } from '$states';
	import Edit from '$lib/pages/Edit.svelte';
	import Info from '$lib/pages/Info.svelte';
	import Settings from '$lib/pages/Settings.svelte';
	import File from '$lib/pages/File.svelte';
	import { Spinner } from '$components';
	import { MessageType } from '$types';
	import { palsData } from '$lib/data';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';

	const appState = getAppState();
	const ws = getSocketState();
	const nav = getNavigationState();
	const toast = getToastState();

	let progressMessage = $state('');
	let errorMessage = $state('');

	$effect(() => {
		if (ws.message && ws.message.type) {
			const { data, type } = ws.message;
			switch (type) {
				case MessageType.ADD_PAL:
					const { player_id, pal } = data;
					if (appState.players && appState.players[player_id] && appState.players[player_id].pals) {
						async function loadPal() {
							const palInfo = await palsData.getPalInfo(pal.character_id);
							pal.name = palInfo?.localized_name || pal.character_id;
							pal.elements = palInfo?.type || [];
							// @ts-ignore
							appState.players[player_id].pals[pal.instance_id] = pal;
							appState.selectedPal = pal;
						}
						loadPal();
						nav.activeTab = 'pal';
					}
					break;
				case MessageType.GET_PLAYERS:
					appState.players = data;
					const numOfPlayers = Object.keys(data).length;
					toast.add(`${numOfPlayers} players loaded successfully`, 'Success!');
					break;
				case MessageType.LOAD_ZIP_FILE:
				case MessageType.LOAD_SAVE_FILE:
					const file = data as { name: string; size: number };
					appState.saveFile = file;
					nav.activePage = 'edit';
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
					nav.activePage = 'file';
					break;
				case MessageType.ERROR:
					errorMessage = data as string;
					nav.activePage = 'error';
					break;
				case MessageType.PROGRESS_MESSAGE:
					progressMessage = data as string;
					break;
				case MessageType.UPDATE_SAVE_FILE:
					const updateMessage = data as string;
					toast.add(updateMessage, 'Success!');
					appState.selectedPlayer = undefined;
					nav.activePage = 'edit';
					break;
			}
			ws.clear(type);
		}
	});
</script>

<div class="flex h-full w-full">
	{#if nav.activePage === 'edit'}
		<Edit />
	{:else if nav.activePage === 'file'}
		<File />
	{:else if nav.activePage === 'info'}
		<Info />
	{:else if nav.activePage === 'settings'}
		<Settings />
	{:else if nav.activePage === 'browser'}
		<div class="flex h-full w-full flex-col items-center justify-center">
			<h2 class="h2 mb-8">🌐 Opened in browser...</h2>
			<span class="mt-2">You can work in either window, just be sure to keep this one open.</span>
		</div>
	{:else if nav.activePage === 'loading'}
		<div class="flex h-full w-full flex-col items-center justify-center">
			<h2 class="h2 mb-8">🤖 Beep Boop, working on it!</h2>
			<Spinner size="size-32" />
			{#if progressMessage}
				<span class="mt-2">{progressMessage}</span>
			{/if}
		</div>
	{:else}
		<div class="flex h-full w-full flex-col items-center justify-center">
			<div class="max-w-2/3 flex flex-col">
				<h1 class="text-4xl font-bold">😵‍💫 Oops... Something went wrong</h1>
				{#if errorMessage}
					<Accordion classes="mt-4 bg-surface-800">
						<Accordion.Item id="error">
							{#snippet control()}
								<h1 class="ml-4 text-3xl font-bold text-red-500">ERROR</h1>
							{/snippet}
							{#snippet panel()}
								<p class="text-lg">{errorMessage}</p>
							{/snippet}
						</Accordion.Item>
					</Accordion>
				{/if}
			</div>
		</div>
	{/if}
</div>
