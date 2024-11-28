<script lang="ts">
	import { getNavigationState, getAppState } from '$states';
	import { Navigation } from '@skeletonlabs/skeleton-svelte';
	import { File, Pencil, Info } from 'lucide-svelte';

	let navigationState = getNavigationState();
	let appState = getAppState();
</script>

<Navigation.Rail width="48px" bind:value={navigationState.activePage}>
	{#snippet tiles()}
		{#if appState.saveFile}
			<Navigation.Tile label="Edit" title="Edit" id="edit" href="/edit" active="bg-secondary-500">
				<Pencil />
			</Navigation.Tile>
		{/if}
		<Navigation.Tile label="Files" title="File" id="file" href="/file" active="bg-secondary-500">
			<File />
		</Navigation.Tile>
		<Navigation.Tile label="About" title="About" id="about" href="/about" active="bg-secondary-500">
			<Info />
		</Navigation.Tile>
	{/snippet}
</Navigation.Rail>
