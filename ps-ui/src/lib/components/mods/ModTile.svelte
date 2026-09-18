<script lang="ts" module>
	/** Literal class names per mod type, so Tailwind keeps them. */
	export const modTypeText: Record<string, string> = {
		ue4ss: 'text-blue-400',
		palschema: 'text-green-400',
		pak: 'text-amber-400',
		logicmods: 'text-purple-400',
		nativedll: 'text-orange-400',
		workshop: 'text-cyan-400',
		hybrid: 'text-pink-400',
		framework: 'text-surface-300'
	};

	const tileTint: Record<string, string> = {
		ue4ss: 'from-blue-500/30',
		palschema: 'from-green-500/30',
		pak: 'from-amber-500/30',
		logicmods: 'from-purple-500/30',
		nativedll: 'from-orange-500/30',
		workshop: 'from-cyan-500/30',
		hybrid: 'from-pink-500/30',
		framework: 'from-surface-500/30'
	};
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { LibraryMod } from '$types';
	import { displayName, initials } from './modList';

	let {
		mod,
		class: className = '',
		muted = false,
		children
	}: { mod: LibraryMod; class?: string; muted?: boolean; children?: Snippet } = $props();
</script>

<div
	class={[
		'to-surface-900 relative flex items-center justify-center overflow-hidden bg-linear-to-br',
		tileTint[mod.mod_type] ?? 'from-surface-500/30',
		muted && 'opacity-50 grayscale',
		className
	]}
>
	<span
		class={[
			'font-extrabold tracking-tight select-none',
			modTypeText[mod.mod_type] ?? 'text-surface-300'
		]}
		aria-hidden="true"
	>
		{initials(displayName(mod))}
	</span>
	{@render children?.()}
</div>
