<script lang="ts">
	import { onMount } from 'svelte';
	import type { Component } from 'svelte';
	import Logo from '$components/layout/Logo.svelte';
	import { SaveDropzone } from '$components/upload';
	import { Button, Tooltip } from '$components/ui';
	import { RotateCcw, Lock } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	interface Props {
		onLoad: (
			zip: Uint8Array,
			name: string,
			source?: { handle?: FileSystemDirectoryHandle; writable?: boolean }
		) => void;
		onResume?: () => void;
		resumeName?: string | null;
	}
	let { onLoad, onResume, resumeName = null }: Props = $props();

	let HeroMap = $state<Component | null>(null);
	onMount(async () => {
		HeroMap = (await import('./HeroMap.svelte')).default;
	});
</script>

<section
	class="relative flex min-h-[72vh] w-full items-center justify-center overflow-hidden py-16"
>
	<div class="absolute inset-0">
		{#if HeroMap}
			{@const Map = HeroMap}
			<Map />
		{/if}
	</div>
	<div class="hero-scrim pointer-events-none absolute inset-0"></div>
	<div class="hero-aurora pointer-events-none absolute inset-0"></div>

	<div class="glass relative z-10 mx-4 w-full max-w-md rounded-2xl p-6 text-center sm:p-8">
		<Logo class="mx-auto mb-5 w-full max-w-xs" />
		<p class="text-surface-200 text-lg">
			{m.landing_hero_tagline()}
		</p>
		<div class="mt-6">
			<SaveDropzone {onLoad} />
		</div>
		<p class="text-surface-300 mt-4 flex items-center justify-center gap-2 text-sm">
			<Lock size={14} />
			{m.landing_hero_privacy()}
		</p>
		{#if resumeName && onResume}
			<Tooltip label={resumeName}>
				<Button variant="secondary" class="mt-4" onclick={onResume}>
					<RotateCcw size={16} />
					{m.landing_hero_resume()}
				</Button>
			</Tooltip>
		{/if}
	</div>
</section>

<style>
	.hero-scrim {
		background: linear-gradient(
			180deg,
			color-mix(in srgb, var(--color-surface-950) 35%, transparent) 0%,
			color-mix(in srgb, var(--color-surface-950) 15%, transparent) 40%,
			color-mix(in srgb, var(--color-surface-950) 85%, transparent) 100%
		);
	}
	.hero-aurora {
		background: radial-gradient(
			700px 240px at 50% -10%,
			color-mix(in srgb, var(--color-primary-300) 16%, transparent),
			transparent 70%
		);
	}
	.glass {
		background: color-mix(in srgb, var(--color-surface-950) 55%, transparent);
		backdrop-filter: blur(10px);
		border: 1px solid color-mix(in srgb, var(--color-surface-400) 16%, transparent);
	}
	@media (prefers-reduced-motion: no-preference) {
		.hero-aurora {
			animation: hero-pulse 8s ease-in-out infinite;
		}
	}
	@keyframes hero-pulse {
		0%,
		100% {
			opacity: 0.55;
		}
		50% {
			opacity: 1;
		}
	}
</style>
