<script lang="ts">
	import { getNavigationState, getAppState, getSocketState } from '$states';
	import { Nav } from '@skeletonlabs/skeleton-svelte';
	import { ExternalLink, File, Pencil, Settings } from 'lucide-svelte';
	import { PUBLIC_DESKTOP_MODE, PUBLIC_WS_URL } from '$env/static/public';
	import { MessageType } from '$types';

	let navigationState = getNavigationState();
	let appState = getAppState();
	let ws = getSocketState();

	function openInBrowser() {
		const url = PUBLIC_WS_URL.replace('/ws', '');
		ws.send(JSON.stringify({ type: MessageType.OPEN_IN_BROWSER, data: url }));
	}
</script>

<Nav.Rail width="48px" bind:value={navigationState.activePage}>
	{#snippet tiles()}
		{#if appState.saveFile}
			<Nav.Tile title="Edit" id="edit" href="/edit" active="bg-secondary-500">
				<Pencil />
			</Nav.Tile>
		{/if}
		<Nav.Tile title="File" id="file" href="/file" active="bg-secondary-500">
			<File />
		</Nav.Tile>
	{/snippet}
	{#snippet footer()}
		{#if PUBLIC_DESKTOP_MODE === 'true' && appState.saveFile}
			<Nav.Tile
				title="Open in Browser"
				id="browser"
				onclick={openInBrowser}
				active="bg-secondary-500"
			>
				<ExternalLink />
			</Nav.Tile>
		{/if}
	{/snippet}
</Nav.Rail>
