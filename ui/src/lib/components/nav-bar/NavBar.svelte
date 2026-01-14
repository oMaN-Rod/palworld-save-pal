<script lang="ts">
	import { getAppState, getModalState } from '$states';
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
		Globe,
		ChevronsRight,
		ChevronsLeft
	} from 'lucide-svelte';

	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { SettingsModal } from '$components/modals';
	import { MessageType } from '$types';
	import { send } from '$lib/utils/websocketUtils';
	import { page } from '$app/state';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { persistedState } from 'svelte-persisted-state';

	let appState = getAppState();
	let modal = getModalState();
	let expanded = persistedState('navbar.expanded', false);

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
			title: m.settings(),
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
	widthExpanded="w-auto"
	value={activeTile}
	onValueChange={() => appState.saveState()}
	expanded={expanded.current}
>
	{#snippet header()}
		<Navigation.Tile
			title={m.toggle_entity({ entity: '' })}
			id="menu"
			onclick={() => (expanded.current = !expanded.current)}
		>
			{#if expanded.current}
				<ChevronsLeft />
			{:else}
				<ChevronsRight />
			{/if}
		</Navigation.Tile>
		{#if appState.saveFile && PUBLIC_DESKTOP_MODE === 'true'}
			<Navigation.Tile
				labelExpanded={c.save}
				title={c.save}
				id="save"
				onclick={() => appState.writeSave()}
			>
				<Save />
			</Navigation.Tile>
		{/if}
	{/snippet}
	{#snippet tiles()}
		<Navigation.Tile
			labelExpanded={m.edit()}
			title={m.edit()}
			id="edit"
			href="/edit"
			active="bg-secondary-500"
		>
			<Pencil />
		</Navigation.Tile>
		{#if PUBLIC_DESKTOP_MODE === 'true'}
			<Navigation.Tile
				labelExpanded={m.file({ count: 2 })}
				title={m.file({ count: 2 })}
				id="file"
				href="/file"
				active="bg-secondary-500"
			>
				<File />
			</Navigation.Tile>
		{:else}
			<Navigation.Tile
				labelExpanded={m.transfer({ count: 1 })}
				title={m.transfer({ count: 1 })}
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
			labelExpanded={m.map()}
			title={m.map()}
			id="map"
			href="/worldmap"
			active="bg-secondary-500"
		>
			<Map />
		</Navigation.Tile>
		<Navigation.Tile
			labelExpanded={m.about()}
			title={m.about()}
			id="about"
			href="/about"
			active="bg-secondary-500"
		>
			<Info />
		</Navigation.Tile>
		<Navigation.Tile
			labelExpanded={c.presets}
			title={c.presets}
			id="presets"
			href="/presets"
			active="bg-secondary-500"
		>
			<FileHeart />
		</Navigation.Tile>
		{#if appState.hasGpsAvailable}
			<Navigation.Tile
				labelExpanded={m.gps()}
				title={m.gps()}
				id="gps"
				href="/gps"
				active="bg-secondary-500"
			>
				<Globe />
			</Navigation.Tile>
		{/if}
		<Navigation.Tile
			labelExpanded={m.ups()}
			title={m.ups()}
			id="ups"
			href="/ups"
			active="bg-secondary-500"
		>
			<Database />
		</Navigation.Tile>
		{#if appState.settings.debug_mode}
			<Navigation.Tile
				labelExpanded={m.debug()}
				title={m.debug()}
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
			labelExpanded={m.settings()}
			title={m.settings()}
			id="settings"
			onclick={handleLanguageSelect}
		>
			<Settings />
		</Navigation.Tile>
	{/snippet}
</Navigation.Rail>
