<script lang="ts">
	import { getNavigationState, getAppState } from '$states';
	import { Nav } from '@skeletonlabs/skeleton-svelte';
	import { File, Pencil, Settings, Info } from 'lucide-svelte';

	let navigationState = getNavigationState();
	let appState = getAppState();
</script>

<Nav.Rail width="48px">
	{#snippet tiles()}
		{#if appState.saveFile}
			<Nav.Tile
				title="Edit"
				id="0"
				onclick={() => (navigationState.activePage = 'Edit')}
				active="bg-secondary-500"
			>
				<Pencil />
			</Nav.Tile>
		{/if}
		<Nav.Tile
			title="File"
			id="1"
			onclick={() => (navigationState.activePage = 'File')}
			active="bg-secondary-500"
		>
			<File />
		</Nav.Tile>
		<!-- Todo: Add Info page with detailed pal info and wiki links 
		
		<Nav.Tile
			title="Info"
			id="2"
			onclick={() => (navigationState.activePage = 'Info')}
			active="bg-secondary-500"
		>
			<Info />
		</Nav.Tile> -->
	{/snippet}
	<!-- Todo: Add settings page, will allow creating profiles and changing settings
	{#snippet footer()}
		<Nav.Tile
			title="settings"
			id="3"
			onclick={() => (navigationState.activePage = 'Settings')}
			active="bg-secondary-500"
		>
			<Settings />
		</Nav.Tile>
	{/snippet} -->
</Nav.Rail>
