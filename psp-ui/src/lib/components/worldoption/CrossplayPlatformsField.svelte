<script lang="ts">
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import { CROSSPLAY_PLATFORMS } from './worldOptionFields';

	let { value, onchange }: { value: string[]; onchange: (next: string[]) => void } = $props();

	function toggle(platform: string, checked: boolean) {
		// Preserve the canonical platform order rather than click order.
		const next = CROSSPLAY_PLATFORMS.map((p) => p.value).filter((p) =>
			p === platform ? checked : value.includes(p)
		);
		onchange(next);
	}
</script>

<div class="grid grid-cols-2 gap-2">
	{#each CROSSPLAY_PLATFORMS as platform (platform.value)}
		<div class="flex items-center justify-between gap-2">
			<span class="text-sm">{platform.label}</span>
			<Switch
				name={platform.value}
				checked={value.includes(platform.value)}
				onCheckedChange={(e: CheckedChangeDetails) => toggle(platform.value, e.checked)}
			/>
		</div>
	{/each}
</div>
