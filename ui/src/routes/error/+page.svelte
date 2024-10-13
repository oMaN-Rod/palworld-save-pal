<script lang="ts">
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { page } from '$app/stores';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$lib/utils/asset-loader';
	import { goto } from '$app/navigation';

	let sadIcon: string = $state('');

	$effect(() => {
		const loadSadIcon = async () => {
			const sadIconPath = `${ASSET_DATA_PATH}/img/icons/Cattiva_Pleading.png`;
			sadIcon = await assetLoader.loadImage(sadIconPath, true);
		};
		loadSadIcon();
	});
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<div class="flex max-w-[1200px] flex-col">
		<div class="flex items-center">
			{#if sadIcon}
				<enhanced:img src={sadIcon} alt="Sad face icon" class="mr-2 h-14 w-14"></enhanced:img>
			{:else}
				<span class="mr-2">😵‍💫</span>
			{/if}
			<h1 class="text-4xl font-bold">Oops... Something went wrong</h1>
		</div>

		<Accordion classes="mt-4 bg-surface-800">
			<Accordion.Item id="error">
				{#snippet control()}
					<h1 class="ml-4 text-3xl font-bold text-red-500">{$page.state.status} ERROR</h1>
				{/snippet}
				{#snippet panel()}
					<p class="text-lg">{$page.state.error.message}</p>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
</div>
