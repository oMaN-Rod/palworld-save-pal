<script lang="ts">
	import { page } from '$app/state';
	import { getNavigationState } from '$states';

	let {
		text,
		key,
		href,
		class: _class
	}: { text: string; class?: string; key: string; href?: string } = $props();

	const nav = getNavigationState();
</script>

<button
	class="hover:bg-secondary-500/50 flex items-center gap-2 rounded p-2 {page.url.pathname == href
		? 'bg-secondary-500/25'
		: ''} {_class}"
	onclick={() => {
		nav.saveAndNavigate(href as string);
	}}
>
	<span>{text}</span>
	<kbd
		class="bg-surface-700/90 border-surface-500 hidden h-4 w-4 items-center justify-center rounded border p-1 text-center text-xs 2xl:flex"
	>
		{key}
	</kbd>
</button>
