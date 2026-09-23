<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { BottomSheet } from '$components/ui/sheet';
	import type { SheetSnap } from '$components/ui/sheet/sheetSnap';
	import { layout } from '$utils/layout.svelte';
	import * as m from '$i18n/messages';

	let {
		title,
		expanded = false,
		onclose,
		children
	}: { title: string; expanded?: boolean; onclose?: () => void; children: Snippet } = $props();

	let snap = $state<SheetSnap>('tall');
</script>

{#if layout.phone}
	{#if expanded}
		<BottomSheet open bind:snap snaps={['half', 'tall']} {title} onClose={() => onclose?.()}>
			{@render children()}
		</BottomSheet>
	{/if}
{:else}
	<div
		class="bg-surface-800/80 text-on-surface h-[calc(100vh-var(--titlebar-h)-84px)] shrink-0 overflow-hidden shadow-lg backdrop-blur-md transition-all duration-300 ease-in-out"
		style:width={expanded ? '420px' : '0px'}
	>
		<div class="flex h-full w-105 flex-col overflow-y-auto p-4">
			<div class="mb-3 flex items-center justify-between">
				<span class="font-semibold">{title}</span>
				<button
					class="hover:text-primary-500 rounded p-1"
					onclick={() => onclose?.()}
					aria-label={m.close_drawer()}
				>
					<Icon icon="tabler:x" class="h-4 w-4" />
				</button>
			</div>
			{@render children()}
		</div>
	</div>
{/if}
