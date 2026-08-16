<script lang="ts">
	import { Popover } from '$components/ui';
	import { theme, themeOptions, type ThemeName } from '$states';
	import Palette from '@lucide/svelte/icons/palette';

	const activeLabel = $derived(
		themeOptions.find((option) => option.value === theme.current)?.label ?? 'Dark'
	);
</script>

<Popover position="bottom-end">
	{#snippet children()}
		<button class="public-chip" type="button" aria-label={activeLabel}>
			<Palette class="h-3.5 w-3.5" />
			<span class="hidden md:inline">{activeLabel}</span>
		</button>
	{/snippet}
	{#snippet content({ close }: { close: () => void })}
		<div class="flex flex-col gap-0.5">
			{#each themeOptions as option (option.value)}
				<button
					type="button"
					class="public-chip-option"
					class:is-active={theme.current === option.value}
					onclick={() => {
						theme.current = option.value as ThemeName;
						close();
					}}
				>
					<span class="theme-swatch" data-theme={option.value}></span>
					<span>{option.label}</span>
				</button>
			{/each}
		</div>
	{/snippet}
</Popover>

<style>
	.theme-swatch {
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 9999px;
		background: var(--color-primary-500);
		border: 1px solid var(--color-surface-600);
	}
</style>
