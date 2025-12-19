<script lang="ts">
	import { getNavigationState, getAppState, type Page, getModalState } from '$states';
	import { Navigation } from '@skeletonlabs/skeleton-svelte';
	import {
		File,
		Pencil,
		Info,
		Upload,
		Settings,
		Save,
		Bug,
		Map,
		FileHeart,
		Download,
		Database,
		Globe
	} from 'lucide-svelte';

	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { SettingsModal } from '$components/modals';
	import { MessageType } from '$types';
	import { send } from '$lib/utils/websocketUtils';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';

	let appState = getAppState();
	let modal = getModalState();

	const activeTile = $derived.by(() => {
		if (page.url.pathname.startsWith('/edit')) return 'edit';
		if (page.url.pathname.startsWith('/file')) return 'file';
		if (page.url.pathname.startsWith('/upload')) return 'upload';
		if (page.url.pathname.startsWith('/worldmap')) return 'map';
		if (page.url.pathname.startsWith('/about')) return 'about';
		if (page.url.pathname.startsWith('/presets')) return 'presets';
		if (page.url.pathname.startsWith('/gps')) return 'gps';
		if (page.url.pathname.startsWith('/ups')) return 'ups';
		if (page.url.pathname.startsWith('/debug')) return 'debug';
		return '';
	});

	async function handleLanguageSelect(): Promise<void> {
		// @ts-ignore
		const result = await modal.showModal<string>(SettingsModal, {
			title: 'Settings',
			settings: appState.settings
		});

		if (result) {
			send(MessageType.UPDATE_SETTINGS, { ...appState.settings });
			setTimeout(() => {
				location.reload();
			}, 500);
		}
	}
</script>

<Navigation.Rail
	width="48px"
	value={activeTile}
	onValueChange={appState.saveState}
>
	{#snippet header()}
		{#if appState.saveFile && PUBLIC_DESKTOP_MODE === 'true'}
			<Navigation.Tile
				label="Save"
				labelBase="text-xs"
				title="Save"
				id="save"
				onclick={() => appState.writeSave()}
			>
				<Save />
			</Navigation.Tile>
		{/if}
	{/snippet}
	{#snippet tiles()}
		<Navigation.Tile
			label="Edit"
			labelBase="text-xs"
			title="Edit"
			id="edit"
			href="/edit"
			active="bg-secondary-500"
		>
			<Pencil />
		</Navigation.Tile>
		{#if PUBLIC_DESKTOP_MODE === 'true'}
			<Navigation.Tile
				label="Files"
				labelBase="text-xs"
				title="File"
				id="file"
				href="/file"
				active="bg-secondary-500"
			>
				<File />
			</Navigation.Tile>
		{:else}
			<Navigation.Tile
				label="Transfer"
				labelBase="text-xs"
				title="Transfer"
				id="upload"
				href="/upload"
				active="bg-secondary-500"
			>
				{#if appState.saveFile}
					<Download />
				{:else}
					<Upload />
				{/if}
			</Navigation.Tile>
		{/if}
		<Navigation.Tile
			label="Map"
			labelBase="text-xs"
			title="Map"
			id="map"
			href="/worldmap"
			active="bg-secondary-500"
		>
			<Map />
		</Navigation.Tile>
		<Navigation.Tile
			label="About"
			labelBase="text-xs"
			title="About"
			id="about"
			href="/about"
			active="bg-secondary-500"
		>
			<Info />
		</Navigation.Tile>
		<Navigation.Tile
			label="Presets"
			labelBase="text-xs"
			title="Presets"
			id="presets"
			href="/presets"
			active="bg-secondary-500"
		>
			<FileHeart />
		</Navigation.Tile>
		{#if appState.gps}
			<Navigation.Tile
				label="GPS"
				labelBase="text-xs"
				title="Global Pal Storage"
				id="gps"
				href="/gps"
				active="bg-secondary-500"
			>
				<Globe />
			</Navigation.Tile>
		{/if}
		<Navigation.Tile
			label="UPS"
			labelBase="text-xs"
			title="Universal Pal Storage"
			id="ups"
			href="/ups"
			active="bg-secondary-500"
		>
			<Database />
		</Navigation.Tile>
		{#if appState.settings.debug_mode}
			<Navigation.Tile
				label="Debug"
				labelBase="text-xs"
				title="Debug"
				id="debug"
				href="/debug"
				active="bg-secondary-500"
			>
				<Bug />
			</Navigation.Tile>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Navigation.Tile
			label="Settings"
			labelBase="text-xs"
			title="Settings"
			id="settings"
			onclick={handleLanguageSelect}
		>
			<Settings />
		</Navigation.Tile>
	{/snippet}
</Navigation.Rail>
