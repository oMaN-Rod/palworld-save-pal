<script lang="ts">
	import { fade } from 'svelte/transition';
	import { getModalState } from '$states';
	import { cn } from '$theme';

	const modal = getModalState();

	let {
		overlayClass = 'bg-black/50',
		contentClass = '',
		rounded = 'rounded',
		children
	} = $props<{
		overlayClass?: string;
		contentClass?: string;
		rounded?: string;
		children: any;
	}>();
</script>

<div>
	{@render children()}
</div>

{#if modal.isOpen}
	<div
		class={cn('fixed inset-0 z-50 flex items-center justify-center', overlayClass)}
		transition:fade={{ duration: 200 }}
	>
		<div class={cn('relative', contentClass, rounded)}>
			<button
				class="absolute right-2 top-2 z-10 text-2xl leading-none hover:opacity-75"
				onclick={() => modal.closeModal()}
			>
				×
			</button>
			<svelte:component this={modal.component} {...modal.props} closeModal={modal.closeModal} />
		</div>
	</div>
{/if}
