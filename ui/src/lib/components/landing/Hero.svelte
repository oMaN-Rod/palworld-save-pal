<script lang="ts">
	import { onMount } from 'svelte';
	import type { Component } from 'svelte';
	import Logo from '$components/layout/Logo.svelte';
	import { SaveDropzone } from '$components/upload';
	import { Button, Tooltip } from '$components/ui';
	import { RotateCcw, Lock } from 'lucide-svelte';

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
			The free, open-source Palworld save editor. In your browser.
		</p>
		<div class="mt-6">
			<SaveDropzone {onLoad} />
		</div>
		<p class="text-surface-300 mt-4 flex items-center justify-center gap-2 text-sm">
			<Lock size={14} /> Your files never leave your device.
		</p>
		{#if resumeName && onResume}
			<Tooltip label={resumeName}>
				<Button variant="secondary" class="mt-4" onclick={onResume}>
					<RotateCcw size={16} /> Resume Previous Save
				</Button>
			</Tooltip>
		{/if}
	</div>
</section>

<style>
	.hero-scrim {
		background: linear-gradient(
			180deg,
			rgba(6, 10, 16, 0.35) 0%,
			rgba(6, 10, 16, 0.15) 40%,
			rgba(6, 10, 16, 0.85) 100%
		);
	}
	.hero-aurora {
		background: radial-gradient(
			700px 240px at 50% -10%,
			rgba(125, 211, 252, 0.16),
			transparent 70%
		);
	}
	.glass {
		background: rgba(9, 13, 20, 0.55);
		backdrop-filter: blur(10px);
		border: 1px solid rgba(148, 163, 184, 0.16);
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
