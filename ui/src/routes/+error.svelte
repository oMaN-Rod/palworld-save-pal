<script lang="ts">
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { page } from '$app/stores';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import * as m from '$i18n/messages';

	let error = $state(['']);
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<div class="max-w-2/3 flex flex-col">
		<h1 class="text-4xl font-bold">😵‍💫 {m.oops_something_wrong()}</h1>
		<Accordion
			value={error}
			onValueChange={(e: ValueChangeDetails) => (error = e.value)}
			classes="mt-4 bg-surface-800"
			collapsible
		>
			<Accordion.Item value="error">
				{#snippet control()}
					<h1 class="ml-4 text-3xl font-bold text-red-500">{$page.status} {m.error()}</h1>
				{/snippet}
				{#snippet panel()}
					<p class="text-lg">{$page.error?.message}</p>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
</div>
