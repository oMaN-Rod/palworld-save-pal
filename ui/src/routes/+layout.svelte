<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal } from '$components';
	import {
		activeSkillsData,
		elementsData,
		itemsData,
		palsData,
		passiveSkillsData,
		presetsData
	} from '$lib/data';
	import { getNavigationState, getSocketState, getAppState, getToastState } from '$states';
	import { MessageType } from '$types';
	import { goto } from '$app/navigation';

	const { children } = $props();

	const ws = getSocketState();
	const appState = getAppState();
	const nav = getNavigationState();
	const toast = getToastState();

	$effect(() => {
		const loadData = async () => {
			await activeSkillsData.getActiveSkills();
			await passiveSkillsData.getPassiveSkills();
			await elementsData.getAllElements();
			await itemsData.getAllItems();
			await palsData.getAllPals();
			await presetsData.getAllPresets();
		};
		loadData();
	});

	$effect(() => {
		if (ws.message && ws.message.type) {
			const { data, type } = ws.message;
			switch (type) {
				case MessageType.ADD_PAL:
					const { player_id, pal } = data;
					if (appState.players && appState.players[player_id] && appState.players[player_id].pals) {
						async function loadPal() {
							const palData = await palsData.getPalInfo(pal.character_id);
							pal.name = palData?.localized_name || pal.character_id;
							pal.elements = palData?.element_types || [];
							// @ts-ignore
							appState.players[player_id].pals[pal.instance_id] = pal;
							appState.selectedPal = pal;
						}
						loadPal();
						nav.activeTab = 'pal';
					}
					ws.clear(type);
					break;
				case MessageType.MOVE_PAL:
					const move_data = data as { player_id: string; pal_id: string; container_id: string };
					if (appState.players && appState.players[move_data.player_id]) {
						const player = appState.players[move_data.player_id];
						const pal = player.pals ? player.pals[move_data.pal_id] : undefined;
						if (pal) {
							pal.storage_id = move_data.container_id;
						}
					}
				case MessageType.LOAD_ZIP_FILE:
				case MessageType.LOAD_SAVE_FILE:
					const file = data as { name: string; size: number };
					appState.saveFile = file;
					ws.clear(type);
					goto('/edit');
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

					ws.clear(type);
					break;
				case MessageType.GET_PLAYERS:
					console.log('Players loaded', data);
					appState.players = data;
					goto('/edit');
					ws.clear(type);
					break;
				case MessageType.UPDATE_SAVE_FILE:
					console.log('Save file updated', data);
					goto('/edit');
					ws.clear(type);
					break;
				case MessageType.ERROR:
					console.error('Error', data);
					goto('/error', {
						state: {
							status: 400,
							error: { message: data }
						}
					});
					ws.clear(type);
					break;
				case MessageType.WARNING:
					console.warn(data);
					toast.add(data, undefined, 'warning');
					ws.clear(type);
			}
		}
	});
</script>

<Toast position="bottom-center" />
<Modal>
	<div class="flex h-screen w-full overflow-hidden">
		<NavBar />
		<main class="flex-1 overflow-hidden">
			{@render children()}
		</main>
	</div>
</Modal>
