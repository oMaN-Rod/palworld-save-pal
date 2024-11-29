<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal } from '$components';
	import {
		activeSkillsData,
		elementsData,
		itemsData,
		palsData,
		passiveSkillsData,
		presetsData,
		expData
	} from '$lib/data';
	import { getNavigationState, getSocketState, getAppState, getToastState } from '$states';
	import { MessageType } from '$types';
	import { goto } from '$app/navigation';
	import { browser } from '$app/environment';
	import { PUBLIC_WS_URL } from '$env/static/public';

	const { children } = $props();

	const ws = getSocketState();
	const appState = getAppState();
	const nav = getNavigationState();
	const toast = getToastState();

	let isPywebview = $state(false);

	function openInBrowser() {
		const url = PUBLIC_WS_URL.replace('/ws', '');
		ws.send(JSON.stringify({ type: MessageType.OPEN_IN_BROWSER, data: url }));
	}

	$effect(() => {
		if (browser) {
			isPywebview = navigator.userAgent.includes('pywebview');
		}
	});

	$effect(() => {
		if (isPywebview) {
			goto('/file');
		}
	});

	$effect(() => {
		const loadData = async () => {
			await activeSkillsData.getActiveSkills();
			await passiveSkillsData.getPassiveSkills();
			await elementsData.getAllElements();
			await itemsData.getAllItems();
			await palsData.getAllPals();
			await presetsData.getAllPresets();
			await expData.getExpData();
		};
		loadData();
	});

	$effect(() => {
		if (ws.message && ws.message.type) {
			const { data, type } = ws.message;
			switch (type) {
				case MessageType.ADD_PAL:
					const { player_id, pal } = data;
					if (!pal) {
						toast.add('Container is full', undefined, 'warning');
						ws.clear(type);
						break;
					}
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
					const move_data = data as {
						player_id: string;
						pal_id: string;
						container_id: string;
						slot_index: number;
					};
					if (appState.players && appState.players[move_data.player_id]) {
						const player = appState.players[move_data.player_id];
						const pal = player.pals ? player.pals[move_data.pal_id] : undefined;
						if (pal) {
							pal.storage_id = move_data.container_id;
							pal.storage_slot = move_data.slot_index;
						}
					}
					ws.clear(type);
					break;
				case MessageType.LOADED_SAVE_FILES:
					const { sav_file_name, players, world_name } = data;
					console.log('Loaded save files', sav_file_name, players);
					appState.resetState();
					appState.saveFile = { name: sav_file_name, world_name };
					appState.playerSaveFiles = players.map((p: any) => ({ name: p }));
					ws.clear(type);
					break;
				case MessageType.SAVE_MODDED_SAVE:
					toast.add(data, 'Saved!', 'success');
					ws.clear(type);
					goto('/file');
					break;
				case MessageType.LOAD_ZIP_FILE:
					const file = data as { name: string; size: number };
					appState.resetState();
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
					goto('/file');
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
					const errorMessage = data as { message: string; trace: string };
					goto('/error', {
						state: {
							message: errorMessage.message,
							trace: errorMessage.trace
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

<Toast position="bottom-center" transition={{ type: 'fly', params: { y: 300 } }} />
<Modal>
	<div class="flex h-screen w-full overflow-hidden">
		<NavBar />
		<main class="flex-1 overflow-hidden">
			{@render children()}
		</main>
	</div>
</Modal>
