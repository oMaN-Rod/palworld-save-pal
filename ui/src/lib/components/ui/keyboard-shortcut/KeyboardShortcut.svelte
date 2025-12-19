<script lang="ts">
	import { page } from "$app/state";

	let { text, key, href, class:_class }: { text: string; class?:string, key: string, href?: string} = $props();
</script>

<a class="flex gap-1 hover:bg-secondary-500/50 p-2 rounded {page.url.pathname == href ? "bg-secondary-500/25": ""} {_class}" {href}>
	<span>{text}</span>
	<kbd class="kbd kbd-xs hidden items-center text-center text-xs 2xl:flex">{key}</kbd>
</a>
