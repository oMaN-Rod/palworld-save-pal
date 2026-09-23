<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';

	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';

	/** Mounted only while open, so a closed panel leaves no focusable search box behind. */
	let {
		id,
		onClose,
		children
	}: {
		id: string;
		onClose: () => void;
		children: Snippet;
	} = $props();

	const label = m.filter({ count: 2 });
</script>

<aside
	{id}
	data-testid="pal-container-filter-panel"
	aria-label={label}
	class="flex w-64 shrink-0 flex-col gap-4 overflow-y-auto xl:w-72"
	transition:slide={{ axis: 'x', duration: 200, easing: cubicOut }}
>
	<div class="flex items-center justify-between gap-2">
		<h3 class="text-sm font-bold">{label}</h3>
		<button
			type="button"
			class="hover:bg-surface-700 flex size-11 shrink-0 items-center justify-center rounded-lg"
			aria-label={m.close()}
			onclick={onClose}
		>
			<Icon icon="tabler:x" class="size-5" />
		</button>
	</div>

	{@render children()}
</aside>
