<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { send } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';

	const isDesktop = PUBLIC_DESKTOP_MODE === 'true';

	// Shown after a choice so the user isn't left staring at a static page while
	// the shell pivots (it closes this window / opens the editor / opens the tray).
	let chosen: 'desktop' | 'browser' | null = $state(null);

	function choose(mode: 'desktop' | 'browser') {
		if (chosen) return;
		chosen = mode;
		// The Rust shell receives `set_mode`, persists the choice, and does the
		// window swap / tray setup — this page just fires it and waits.
		send(MessageType.SET_MODE, { mode });
	}
</script>

<div class="flex h-full w-full flex-col items-center justify-center gap-6 overflow-y-auto p-6">
	<div class="flex items-center gap-3">
		<Icon icon="tabler:rocket" size={28} class="text-primary-400" />
		<div>
			<h2 class="text-lg leading-tight font-bold">Palworld Save Pal</h2>
			<p class="text-muted text-xs tracking-wider uppercase">Choose how to open it</p>
		</div>
	</div>

	{#if !isDesktop}
		<div class="max-w-sm text-center">
			<Icon icon="tabler:info-circle" size={40} class="text-surface-400 mx-auto mb-3" />
			<p class="text-muted text-sm">
				Display-mode selection is part of the Linux desktop app. Open the desktop version to
				choose how it launches.
			</p>
		</div>
	{:else if chosen}
		<div class="flex flex-col items-center gap-3 py-8 text-center">
			<Icon icon="tabler:check" size={40} class="text-green-400" />
			<p class="text-surface-50 text-sm font-medium">
				Opening in {chosen === 'desktop' ? 'Desktop Mode' : 'System Tray / Browser Mode'}…
			</p>
			<p class="text-muted text-xs">You can switch this anytime from Settings or the tray icon.</p>
		</div>
	{:else}
		<p class="text-muted max-w-sm text-center text-xs">
			On Linux the editor runs in WebKitGTK, which can feel slower than a full browser.
			Choose the mode you prefer — you can change it later.
		</p>

		<div class="grid w-full max-w-sm gap-3">
			<button
				type="button"
				onclick={() => choose('desktop')}
				class="border-surface-600/60 hover:border-primary-400 bg-surface-900 group flex cursor-pointer items-start gap-3 rounded-lg border p-4 text-left transition-colors"
			>
				<Icon icon="tabler:device-desktop" size={26} class="text-primary-300 mt-0.5 shrink-0" />
				<div>
					<p class="text-surface-50 text-sm font-bold">Desktop Mode</p>
					<p class="text-muted mt-0.5 text-xs">
						The familiar app window. Recommended if you have a capable GPU and prefer a
						dedicated window.
					</p>
				</div>
			</button>

			<button
				type="button"
				onclick={() => choose('browser')}
				class="border-surface-600/60 hover:border-primary-400 bg-surface-900 group flex cursor-pointer items-start gap-3 rounded-lg border p-4 text-left transition-colors"
			>
				<Icon icon="tabler:world" size={26} class="text-primary-300 mt-0.5 shrink-0" />
				<div>
					<p class="text-surface-50 text-sm font-bold">System Tray / Browser Mode</p>
					<p class="text-muted mt-0.5 text-xs">
						Runs quietly in the background and opens the editor in your browser — the fastest,
						smoothest option. Control it from the tray icon.
					</p>
				</div>
			</button>
		</div>

		<p class="text-muted text-center text-[11px]">
			WebKitGTK can be slower on integrated or virtual GPUs; the browser build typically runs at
			full speed.
		</p>
	{/if}
</div>
