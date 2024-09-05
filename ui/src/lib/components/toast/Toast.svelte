<script lang="ts">
	import { getToastState } from '../../states/toastState.svelte';
	import ToastItem from './ToastItem.svelte';
	import type { ToastPosition, ToastTransition } from '$types';

	let {
		position = 'top-right',
		transition = { type: 'fly', params: { x: 300 } }
	}: {
		position?: ToastPosition;
		transition?: ToastTransition;
	} = $props();

	const toastState = getToastState();
	$effect(() => {
		toastState.setPosition(position);
		toastState.setTransition(transition);
	});

	let positionClasses = $derived(
		{
			'top-center': 'top-4 left-1/2 transform -translate-x-1/2',
			'top-right': 'top-4 right-4',
			'top-left': 'top-4 left-4',
			'bottom-right': 'bottom-4 right-4',
			'bottom-left': 'bottom-4 left-4',
			'bottom-center': 'bottom-4 left-1/2 transform -translate-x-1/2'
		}[toastState.position as string]
	);
</script>

<div class="fixed z-50 flex flex-col gap-2 {positionClasses}">
	{#each toastState.toasts as toast (toast.id)}
		<ToastItem {toast} />
	{/each}
</div>
