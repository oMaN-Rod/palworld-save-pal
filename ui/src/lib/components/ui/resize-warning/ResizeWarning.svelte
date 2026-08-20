<script lang="ts">
	import Maximize from '@lucide/svelte/icons/maximize';
	import { getAppState } from '$states';
	import { isWebBuild } from '$lib/utils/platform';

	// z-[99999] must stay above every other overlay (Modal 50000, PalEditorOverlay 40000).
	// The public shell (web build without a loaded save: landing, map, wiki, breeding) is exempt -- those pages are meant to be browsable on phones.
	const MIN_WIDTH = 800;
	const MIN_HEIGHT = 500;

	const appState = getAppState();
	const isPublicShell = $derived(isWebBuild && !appState.saveFile);

	let winW = $state(0);
	let winH = $state(0);

	$effect(() => {
		function update() {
			winW = window.innerWidth;
			winH = window.innerHeight;
		}
		update();
		window.addEventListener('resize', update);
		return () => window.removeEventListener('resize', update);
	});

	// `winW > 0` suppresses a false positive before the first measure runs.
	let tooSmall = $derived(winW > 0 && (winW < MIN_WIDTH || winH < MIN_HEIGHT));
</script>

{#if tooSmall && !isPublicShell}
	<div
		class="fixed inset-0 z-[99999] flex animate-fade-in flex-col items-center justify-center bg-surface-950 px-6 text-center"
		role="alert"
	>
		<Maximize class="text-warning-400 mb-3 size-12 shrink-0" />
		<h2 class="mb-1 text-lg font-bold text-surface-50">Window Too Small</h2>
		<p class="max-w-xs text-xs text-surface-400">
			Please resize to at least {MIN_WIDTH}×{MIN_HEIGHT} px.
		</p>
	</div>
{/if}
