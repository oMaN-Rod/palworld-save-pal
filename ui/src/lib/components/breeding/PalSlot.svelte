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

	const dims = $derived({ sm: 'w-8 h-8', md: 'w-10 h-10', lg: 'w-14 h-14' }[size]);
	const textSize = $derived({ sm: 'text-sm', md: 'text-base', lg: 'text-lg' }[size]);
	const shown = $derived(display || tribe);
	const iconSrc = $derived(assetLoader.loadMenuImage(characterId ?? tribe));
	const GenderIcon = $derived(gender === 'Male' ? Mars : gender === 'Female' ? Venus : null);
	const genderColor = $derived(
		gender === 'Male' ? 'text-primary-300' : gender === 'Female' ? 'text-tertiary-400' : ''
	);
</script>

<div class="flex min-w-0 items-center gap-2">
	<div class="relative shrink-0">
		<img
			src={iconSrc}
			alt={shown}
			class="{dims} rounded-sm border-surface-600 bg-surface-900 border object-contain"
			loading="lazy"
		/>
		{#if GenderIcon}
			<div class="absolute -right-0.5 -bottom-0.5 {genderColor} bg-surface-900 rounded-full">
				<GenderIcon size={12} />
			</div>
		{/if}
	</div>
	<div class="min-w-0">
		<p class="{textSize} text-surface-50 truncate leading-tight font-medium">{shown}</p>
	</div>
</div>
