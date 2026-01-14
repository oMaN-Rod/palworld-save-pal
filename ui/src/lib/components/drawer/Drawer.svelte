<script lang="ts">
	import { cn } from '$theme';
	import { ArrowLeft, ArrowRight } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	type Position = 'left' | 'right';
	type TabPosition = 'start' | 'center' | 'end';

	let {
		position = 'left',
		tabPosition = 'start',
		class: className = '',
		width = '400px',
		initiallyExpanded = true,
		children
	} = $props<{
		position?: Position;
		tabPosition?: TabPosition;
		class?: string;
		width?: string;
		initiallyExpanded?: boolean;
		children: any;
	}>();

	let isOpen = $state(initiallyExpanded);

	const toggleDrawer = () => {
		isOpen = !isOpen;
	};

	const drawerClass = $derived(
		cn(
			'h-full transition-all duration-300 ease-in-out bg-surface-800 text-on-surface shadow-lg',
			isOpen ? 'w-full' : 'w-0',
			className
		)
	);

	const tabClass = $derived(
		cn(
			'absolute bg-surface-800 text-on-primary cursor-pointer transition-all duration-300 ease-in-out p-2',
			position === 'left' ? 'left-full' : 'right-full',
			tabPosition === 'start' && 'top-0',
			tabPosition === 'center' && 'top-1/2 -translate-y-1/2',
			tabPosition === 'end' && 'bottom-0'
		)
	);

	$effect(() => {
		document.documentElement.style.setProperty('--drawer-width', isOpen ? width : '0px');
	});
</script>

<div class="relative h-full">
	<div class={drawerClass}>
		{#if isOpen}
			<div class="h-full overflow-y-auto p-4">
				{@render children()}
			</div>
		{/if}
	</div>
	<button
		class={tabClass}
		onclick={toggleDrawer}
		aria-label={isOpen ? m.close_drawer() : m.open_drawer()}
	>
		{#if position === 'left'}
			{#if isOpen}
				<ArrowLeft />
			{:else}
				<ArrowRight />
			{/if}
		{:else if isOpen}
			<ArrowRight />
		{:else}
			<ArrowLeft />
		{/if}
	</button>
</div>

<style>
	:global(:root) {
		--drawer-width: 0px;
	}
</style>
