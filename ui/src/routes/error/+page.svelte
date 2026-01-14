<script lang="ts">
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { page } from '$app/stores';
	import { staticIcons } from '$types/icons';
	import { Copy } from 'lucide-svelte';
	import { getToastState } from '$states';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	const toast = getToastState();

	interface Error {
		message: string;
		trace: string;
	}

	const error = $derived($page.state) as Error;
	let errorExpand = $state(['']);

	function copyErrorToClipboard() {
		const errorText = `${error.message}\n\n${error.trace}`;
		navigator.clipboard
			.writeText(errorText)
			.then(() => {
				toast.add(m.error_copied(), m.success(), 'success');
			})
			.catch((err) => {
				console.error('Failed to copy error details: ', err);
				toast.add(m.error_copy_failed(), m.error(), 'error');
			});
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<div class="flex w-[1080px] flex-col">
		<div class="flex items-center">
			<img src={staticIcons.sadIcon} alt="Sad face icon" class="mr-2 h-14 w-14" />
			<h1 class="text-4xl font-bold">{m.oops_something_wrong()}</h1>
		</div>

		<Accordion
			value={errorExpand}
			onValueChange={(e: ValueChangeDetails) => (errorExpand = e.value)}
			classes="mt-4 bg-surface-800"
			collapsible
		>
			<Accordion.Item value="error">
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
