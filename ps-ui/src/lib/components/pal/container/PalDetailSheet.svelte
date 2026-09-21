<script lang="ts">
	import type { Snippet } from 'svelte';

	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { sheetActions, type ActionDescriptor } from '$components/ui/actions';
	import BottomSheet from '$components/ui/sheet/BottomSheet.svelte';

	let {
		open = $bindable(false),
		title,
		subtitle,
		actions = [],
		onClose,
		children
	}: {
		open?: boolean;
		title: string;
		subtitle?: string;
		actions?: ActionDescriptor[];
		onClose: () => void;
		children?: Snippet;
	} = $props();

	const rows = $derived(sheetActions(actions));

	async function activate(action: ActionDescriptor): Promise<void> {
		try {
			await action.run();
		} finally {
			onClose();
		}
	}
</script>

<BottomSheet {open} {title} {onClose} snaps={['peek', 'tall']}>
	{#if subtitle}
		<p class="text-surface-400 mb-2 text-xs">{subtitle}</p>
	{/if}

	{#if children}
		<div class="border-surface-600/60 mb-2 border-b pb-3">
			{@render children()}
		</div>
	{/if}

	{#if rows.length > 0}
		<ul class="flex flex-col">
			{#each rows as action (action.id)}
				<li>
					<button
						type="button"
						class="border-surface-700/40 flex min-h-[52px] w-full items-center gap-3.5 border-b px-1 text-left last:border-b-0"
						class:text-error-300={action.danger}
						onclick={() => activate(action)}
					>
						<Icon icon={action.icon} class="size-5 shrink-0" />
						<span class="flex-1 text-sm">{action.label}</span>
						{#if action.detail}
							<span class="text-surface-400 text-sm font-bold">{action.detail()}</span>
						{/if}
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</BottomSheet>
