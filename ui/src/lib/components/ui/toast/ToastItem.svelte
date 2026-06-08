<script lang="ts">
	import type { ToastType } from '$types';
	import { X } from 'lucide-svelte';
	import { getToastState } from '$states/toastState.svelte';
	import { fly, scale } from 'svelte/transition';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import Button from '../button/Button.svelte';

	let { toast }: { toast: ToastType } = $props();

	const toastState = getToastState();

	let colorClasses = $derived(
		{
			default: 'bg-primary-700/90 border-primary-600/50 shadow-glow-cyan',
			success: 'bg-success-700/90 border-success-600/50 shadow-glow-toxic-sm',
			error: 'bg-error-700/90 border-error-600/50 shadow-glow-red',
			warning: 'bg-warning-700/90 border-warning-600/50 shadow-glow-amber',
			info: 'bg-secondary-700/90 border-secondary-600/50 shadow-glow-gold'
		}[toast.color || 'default']
	);

	let transition = $derived(toastState.transition.type === 'fly' ? fly : scale);
	let transitionParams = $derived(toastState.transition.params || {});
</script>

<div
	class="relative flex min-h-16 max-w-1/2 min-w-60 flex-col items-center justify-center rounded-md border p-4 text-white shadow-md backdrop-blur-sm {colorClasses}"
	transition:transition={transitionParams}
>
	<span class="font-bold">{toast.title}</span>
	<span>{toast.message}</span>
	<Button
		variant="ghost"
		size="icon"
		class="absolute top-1 right-1 size-5"
		onclick={() => toastState.remove(toast.id)}
	>
		<span class="sr-only">{m.close_toast()}</span>
		<X class="size-4" />
	</Button>
</div>
