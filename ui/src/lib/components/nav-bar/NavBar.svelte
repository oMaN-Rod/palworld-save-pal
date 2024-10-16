<script lang="ts">
	import { getNavigationState, getAppState } from '$states';
	import { Nav } from '@skeletonlabs/skeleton-svelte';
	import { File, Pencil } from 'lucide-svelte';
	import { page } from '$app/stores';

	let navigationState = getNavigationState();
	let appState = getAppState();

	$effect(() => {
		const currentPath = $page.url.pathname;
		if (currentPath === '/edit') {
			navigationState.activePage = 'edit';
		} else if (currentPath === '/file') {
			navigationState.activePage = 'file';
		}
	});
</script>

<Nav.Rail width="48px">
	{#snippet tiles()}
		{#if appState.saveFile}
			<Nav.Tile
				title="Edit"
				id="edit"
				href="/edit"
				active="bg-secondary-500"
				selected={navigationState.activePage === 'edit'}
			>
				<Pencil />
			</Nav.Tile>
		{/if}
		<Nav.Tile
			title="File"
			id="file"
			href="/file"
			active="bg-secondary-500"
			selected={navigationState.activePage === 'file'}
		>
			<File />
		</Nav.Tile>
	{/snippet}
</Nav.Rail>
