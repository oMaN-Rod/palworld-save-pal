<script lang="ts">
	import type { Snippet } from 'svelte';
	import { BottomSheet } from '$components/ui/sheet';
	import type { SheetSnap } from '$components/ui/sheet/sheetSnap';
	import { layout } from '$utils/layout.svelte';

	let {
		title,
		onClose,
		active = true,
		class: klass = '',
		children
	}: {
		title: string;
		onClose: () => void;
		/** Only gates the phone sheet; the desktop pane always renders. */
		active?: boolean;
		class?: string;
		children: Snippet;
	} = $props();

	let snap = $state<SheetSnap>('tall');
</script>

{#if layout.phone}
	{#if active}
		<BottomSheet open bind:snap snaps={['half', 'tall']} {title} {onClose}>
			{@render children()}
		</BottomSheet>
	{/if}
{:else}
	<div class={klass}>
		{@render children()}
	</div>
{/if}
