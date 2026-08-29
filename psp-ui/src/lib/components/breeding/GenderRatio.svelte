<script lang="ts">
	// Identity never rests on hue alone -- each side keeps its 'ph:gender-male'/'ph:gender-female' glyph, so the split still reads under CVD or in a greyscale export.
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';

	let {
		prob,
		size = 'md'
	}: {
		prob: { male: number; female: number };
		size?: 'sm' | 'md';
	} = $props();

	const male = $derived(Math.round(prob.male * 100));
	const female = $derived(100 - male);
	const even = $derived(male === female);
	const numCls = $derived(size === 'sm' ? 'text-xs' : 'text-sm');
	const iconSize = $derived(size === 'sm' ? 11 : 13);
</script>

<div
	class="flex shrink-0 flex-col items-end gap-1"
	title="{m.breeding_gender_probability()} — {m.breeding_male()} {male}% / {m.breeding_female()} {female}%"
>
	<div class="flex items-center gap-2 leading-none font-bold tabular-nums {numCls}">
		{#if male > 0}
			<span class="text-primary-300 flex items-center gap-0.5">
				<Icon icon="ph:gender-male" size={iconSize} class="shrink-0" />{male}%
			</span>
		{/if}
		{#if female > 0}
			<span class="text-tertiary-300 flex items-center gap-0.5">
				<Icon icon="ph:gender-female" size={iconSize} class="shrink-0" />{female}%
			</span>
		{/if}
	</div>

	<!-- Meter: 2px surface gap between segments so they read as two marks. -->
	<div class="bg-surface-800/60 flex h-1 w-full overflow-hidden rounded-full">
		{#if male > 0}
			<div class="bg-primary-400 h-full" style:width="{male}%"></div>
		{/if}
		{#if male > 0 && female > 0}
			<div class="bg-surface-950 h-full w-[2px] shrink-0"></div>
		{/if}
		{#if female > 0}
			<div class="bg-tertiary-400 h-full flex-1"></div>
		{/if}
	</div>

	{#if !even}
		<span class="text-surface-400 text-[10px] leading-none font-medium">
			{male > female ? m.breeding_male_skewed() : m.breeding_female_skewed()}
		</span>
	{/if}
</div>
