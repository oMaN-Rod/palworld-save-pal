<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '$theme';
	import Spinner from '../spinner/Spinner.svelte';
	import { buttonClasses, type ButtonVariant, type ButtonSize } from './button.styles';

	let {
		variant = 'neutral',
		size = 'md',
		type = 'button',
		href = undefined,
		loading = false,
		disabled = false,
		class: className = '',
		children,
		...rest
	}: {
		variant?: ButtonVariant;
		size?: ButtonSize;
		type?: 'button' | 'submit' | 'reset';
		href?: string;
		loading?: boolean;
		disabled?: boolean;
		class?: string;
		children?: Snippet;
		[key: string]: any;
	} = $props();

	let isDisabled = $derived(disabled || loading);
	let classes = $derived(cn(buttonClasses({ variant, size }), className));
</script>

{#if href}
	<a
		{...rest}
		href={isDisabled ? undefined : href}
		class={classes}
		role="button"
		aria-disabled={isDisabled}
		aria-busy={loading}
	>
		{#if loading}<Spinner size="size-4" />{/if}
		{@render children?.()}
	</a>
{:else}
	<button {...rest} {type} class={classes} disabled={isDisabled} aria-busy={loading}>
		{#if loading}<Spinner size="size-4" />{/if}
		{@render children?.()}
	</button>
{/if}
