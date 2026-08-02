<script lang="ts">
	import type { Snippet } from "svelte";


    let { href, children }: { href: string; children: Snippet; } = $props();
</script>

<a
	{href}
	target="_blank"
	rel="noopener noreferrer"
	aria-label="GitHub"
	class="flex items-center gap-2 hover:ring-2 hover:ring-secondary-500/50 hover:ring-offset-2 hover:ring-offset-surface-900/10"
>
	{@render children()}
</a>
