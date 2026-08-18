<script lang="ts">
	import type { Snippet } from 'svelte';

	let { href, label, children }: { href: string; label: string; children: Snippet } = $props();
</script>

<a
	{href}
	target="_blank"
	rel="noopener noreferrer"
	aria-label={label}
	class="hover:ring-secondary-500/50 hover:ring-offset-surface-900/10 flex items-center gap-2 hover:ring-2 hover:ring-offset-2"
>
	{@render children()}
</a>
