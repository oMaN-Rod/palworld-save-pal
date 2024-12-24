<script lang="ts">
	import {
		getNavigationState,
		getAppState,
		type Page,
		getModalState,
		getSocketState
	} from '$states';
	import { Navigation } from '@skeletonlabs/skeleton-svelte';
	import { File, Pencil, Info, Upload, Languages } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { LanguageSelectModal } from '$components/modals';
	import { MessageType } from '$types';
	import { bootstrap } from '$lib/data';

	let navigationState = getNavigationState();
	let appState = getAppState();
	let modal = getModalState();
	let ws = getSocketState();

	page.subscribe((value) => {
		const { id } = value.route;
		navigationState.activePage = id?.replace('/', '') as Page;
	});

	async function handleLanguageSelect(): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<string>(LanguageSelectModal, {
			title: 'Choose Language',
			value: appState.settings.language
		});

		if (result) {
			// Handle language change
			const message = {
				type: MessageType.UPDATE_SETTINGS,
				data: {
					language: result
				}
			};
			ws.send(JSON.stringify(message));
			setTimeout(() => {
				location.reload();
			}, 500);
		}
	}
</script>

<Navigation.Rail width="48px" bind:value={navigationState.activePage}>
	{#snippet tiles()}
		{#if appState.saveFile}
			<Navigation.Tile label="Edit" title="Edit" id="edit" href="/edit" active="bg-secondary-500">
				<Pencil />
			</Navigation.Tile>
		{/if}
		{#if PUBLIC_DESKTOP_MODE}
			<Navigation.Tile label="Files" title="File" id="file" href="/file" active="bg-secondary-500">
				<File />
			</Navigation.Tile>
		{:else}
			<Navigation.Tile
				label="Upload"
				title="Upload"
				id="upload"
				href="/upload"
				active="bg-secondary-500"
			>
				<Upload />
			</Navigation.Tile>
		{/if}
		<Navigation.Tile label="About" title="About" id="about" href="/about" active="bg-secondary-500">
			<Info />
		</Navigation.Tile>
	{/snippet}
	{#snippet footer()}
		<Navigation.Tile label="Lang" title="Language" id="language" onclick={handleLanguageSelect}>
			<Languages />
		</Navigation.Tile>
	{/snippet}
</Navigation.Rail>
