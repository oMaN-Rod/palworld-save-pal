<script lang="ts">
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { page } from '$app/stores';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$lib/utils/asset-loader';
	import { Copy } from 'lucide-svelte';
	import { getToastState } from '$states';

	let sadIcon: string = $state('');
	const toast = getToastState();

	$effect(() => {
		const loadSadIcon = async () => {
			const sadIconPath = `${ASSET_DATA_PATH}/img/icons/Cattiva_Pleading.png`;
			sadIcon = await assetLoader.loadImage(sadIconPath, true);
		};
		loadSadIcon();
	});

	interface Error {
		message: string;
		trace: string;
	}

	const error = $derived($page.state) as Error;

	function copyErrorToClipboard() {
		const errorText = `${error.message}\n\n${error.trace}`;
		navigator.clipboard
			.writeText(errorText)
			.then(() => {
				toast.add('Error details copied to clipboard', 'Success', 'success');
			})
			.catch((err) => {
				console.error('Failed to copy error details: ', err);
				toast.add('Failed to copy error details', 'Error', 'error');
			});
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<div class="flex w-[1080px] flex-col">
		<div class="flex items-center">
			{#if sadIcon}
				<enhanced:img src={sadIcon} alt="Sad face icon" class="mr-2 h-14 w-14"></enhanced:img>
			{:else}
				<span class="mr-2">😵‍💫</span>
			{/if}
			<h1 class="text-4xl font-bold">Oops... Something went wrong</h1>
		</div>

		<Accordion classes="mt-4 bg-surface-800" collapsible>
			<Accordion.Item id="error">
				{#snippet control()}
					<div class="flex w-full items-center justify-between">
						<h1 class="ml-4 text-3xl font-bold text-red-500">
							{error.message ? error.message.slice(0, 64) : '🤷‍♂️'}...
						</h1>
						<button
							class="btn btn-sm variant-filled-secondary"
							onclick={copyErrorToClipboard}
							aria-label="Copy error details to clipboard"
						>
							<Copy size={20} />
						</button>
					</div>
				{/snippet}
				{#snippet panel()}
					<div class="max-h-[400px] overflow-scroll">
						<span class="font-bold">{error.message}</span>
						<pre class="text-lg">{error.trace}</pre>
					</div>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
</div>
