<script lang="ts">
	// Small reusable pal icon + name pill. Used in chain steps, direct results,
	// and picker selections. Renders an icon (via assetLoader) plus the display
	// name, with a subtle border/bg treatment.
	import { assetLoader } from '$lib/utils/assetLoader';
	import Mars from '@lucide/svelte/icons/mars';
	import Venus from '@lucide/svelte/icons/venus';

	let {
		tribe,
		display,
		characterId,
		size = 'md',
		gender = null
	}: {
		tribe: string;
		display?: string | null;
		characterId?: string;
		size?: 'sm' | 'md' | 'lg';
		gender?: string | null;
	} = $props();

	const dims = $derived({ sm: 'w-5 h-5', md: 'w-8 h-8', lg: 'w-12 h-12' }[size]);
	const textSize = $derived({ sm: 'text-[10px]', md: 'text-xs', lg: 'text-sm' }[size]);
	const shown = $derived(display || tribe);
	const iconSrc = $derived(assetLoader.loadPalImage(characterId ?? tribe));
	const GenderIcon = $derived(gender === 'Male' ? Mars : gender === 'Female' ? Venus : null);
	const genderColor = $derived(gender === 'Male' ? 'text-sky-400' : gender === 'Female' ? 'text-pink-400' : '');
</script>

<div class="flex items-center gap-1.5 min-w-0">
	<div class="relative shrink-0">
		<img
			src={iconSrc}
			alt={shown}
			class="{dims} object-contain rounded-2 border border-surface-600 bg-surface-900"
			loading="lazy"
		/>
		{#if GenderIcon}
			<div class="absolute -bottom-0.5 -right-0.5 {genderColor} bg-surface-900 rounded-full">
				<GenderIcon size={10} />
			</div>
		{/if}
	</div>
	<div class="min-w-0">
		<p class="{textSize} font-medium text-surface-50 truncate leading-tight">{shown}</p>
		{#if display && display !== tribe}
			<p class="text-[9px] text-surface-400 font-mono truncate leading-tight">{tribe}</p>
		{/if}
	</div>
</div>
