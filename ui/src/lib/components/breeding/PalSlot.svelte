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
	const genderColor = $derived(
		gender === 'Male' ? 'text-primary-300' : gender === 'Female' ? 'text-tertiary-400' : ''
	);
</script>

<div class="flex min-w-0 items-center gap-1.5">
	<div class="relative shrink-0">
		<img
			src={iconSrc}
			alt={shown}
			class="{dims} rounded-sm border-surface-600 bg-surface-900 border object-contain"
			loading="lazy"
		/>
		{#if GenderIcon}
			<div class="absolute -right-0.5 -bottom-0.5 {genderColor} bg-surface-900 rounded-full">
				<GenderIcon size={10} />
			</div>
		{/if}
	</div>
	<div class="min-w-0">
		<p class="{textSize} text-surface-50 truncate leading-tight font-medium">{shown}</p>
		{#if display && display !== tribe}
			<p class="text-surface-400 truncate font-mono text-[9px] leading-tight">{tribe}</p>
		{/if}
	</div>
</div>
