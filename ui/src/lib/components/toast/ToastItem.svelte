<script lang="ts">
	import type { ToastType } from '$types';
	import { X } from 'lucide-svelte';
	import { getToastState } from '$states/toastState.svelte';
	import { fly, scale } from 'svelte/transition';

	let { toast }: { toast: ToastType } = $props();

	const toastState = getToastState();

	let colorClasses = $derived(
		{
			default: 'bg-primary-700 border-primary-800',
			success: 'bg-success-700 border-success-800',
			error: 'bg-error-700 border-error-800',
			warning: 'bg-warning-700 border-warning-800 text-surface-800',
			info: 'bg-secondary-700 border-secondary-800'
		}[toast.color || 'default']
	);

	let transition = $derived(toastState.transition.type === 'fly' ? fly : scale);
	let transitionParams = $derived(toastState.transition.params || {});
</script>

<div
	class="relative flex min-h-16 min-w-60 flex-col items-center justify-center rounded-md border p-2 shadow-md {colorClasses}"
	transition:transition={transitionParams}
>
	<span class="font-bold">{toast.title}</span>
	<span>{toast.message}</span>
	<button class="absolute right-2 top-2 size-5" onclick={() => toastState.remove(toast.id)}>
		<span class="sr-only">Close toast</span>
		<X class="size-4" />
	</button>
</div>
