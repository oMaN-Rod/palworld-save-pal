<script lang="ts">
	import { Search, X } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	let { value = $bindable(''), placeholder = `${m.docs_search_placeholder()}` }: { value?: string; placeholder?: string } = $props();
</script>

<div class="relative">
	<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-400" />
	<input
		type="text"
		bind:value
		{placeholder}
		class="w-full rounded-md border border-surface-700 bg-surface-800 py-2 pl-10 pr-10 text-sm text-surface-50 placeholder:text-surface-500 focus:border-primary-500 focus:outline-none"
	/>
	{#if value}
		<button
			class="absolute right-3 top-1/2 -translate-y-1/2 text-surface-400 hover:text-surface-200"
			onclick={() => (value = '')}
		>
			<X class="h-4 w-4" />
		</button>
	{/if}
</div>
