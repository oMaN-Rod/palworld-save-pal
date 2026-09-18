<script lang="ts">
	import type { NexusModSummary } from '$types';
	import { safeHttpUrl } from './nexusSafety';

	let { mod }: { mod: NexusModSummary } = $props();

	let failed = $state(false);
	const src = $derived(safeHttpUrl(mod.thumbnail_url) ?? safeHttpUrl(mod.picture_url));
	const initials = $derived(
		mod.name
			.split(/\s+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((word) => word[0]?.toUpperCase() ?? '')
			.join('')
	);

	$effect(() => {
		src;
		failed = false;
	});
</script>

{#if src && !failed}
	<img
		{src}
		alt=""
		loading="lazy"
		class="h-full w-full object-cover"
		onerror={() => (failed = true)}
	/>
{:else}
	<div
		class="flex h-full w-full items-center justify-center bg-gradient-to-br from-slate-700 to-slate-900 text-lg font-semibold text-white/80"
	>
		{initials}
	</div>
{/if}
