<script lang="ts">
	import Search from '@lucide/svelte/icons/search';
	import X from '@lucide/svelte/icons/x';
	import * as m from '$i18n/messages';

	let {
		value = $bindable(''),
		placeholder = `${m.docs_search_placeholder()}`
	}: { value?: string; placeholder?: string } = $props();
</script>

<div class="relative">
	<Search class="text-surface-400 absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2" />
	<input
		type="text"
		bind:value
		{placeholder}
		class="border-surface-700 bg-surface-800 text-surface-50 placeholder:text-surface-500 focus:border-primary-500 w-full rounded-md border py-2 pr-10 pl-10 text-sm focus:outline-none"
	/>
	{#if value}
		<button
			class="text-surface-400 hover:text-surface-200 absolute top-1/2 right-3 -translate-y-1/2"
			onclick={() => (value = '')}
		>
			<X class="h-4 w-4" />
		</button>
	{/if}
</div>
