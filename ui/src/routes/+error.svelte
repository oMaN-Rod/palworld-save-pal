<script lang="ts">
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { page } from '$app/stores';
</script>

<div class="flex h-full w-full flex-col items-center justify-center">
	<div class="max-w-2/3 flex flex-col">
		<h1 class="text-4xl font-bold">😵‍💫 Oops... Something went wrong</h1>
		<Accordion classes="mt-4 bg-surface-800">
			<Accordion.Item id="error">
				{#snippet control()}
					<h1 class="ml-4 text-3xl font-bold text-red-500">{$page.status} ERROR</h1>
				{/snippet}
				{#snippet panel()}
					<p class="text-lg">{$page.error?.message}</p>
				{/snippet}
			</Accordion.Item>
		</Accordion>
	</div>
</div>
