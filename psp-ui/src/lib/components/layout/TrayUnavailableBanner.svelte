<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { send, sendAndWait } from '$utils/websocketUtils';
	import { MessageType } from '$types';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { onMount } from 'svelte';

	// Linux browser-mode fallback only. The desktop build bakes this flag and
	// the shell reports at runtime (via get_display_mode) whether it runs a
	// mode-switching shell, is in browser mode, and could actually display its
	// tray icon. Everything else — web build, Docker, Windows/macOS desktop —
	// keeps `supported: false` and never sees the banner.
	const isDesktop = PUBLIC_DESKTOP_MODE === 'true';

	// Linux mode-select/tray strings are deliberately hardcoded English, like
	// the rest of the shell-mode UI (see the mode-select page and Settings'
	// display-mode section).
	let visible = $state(false);
	let quitting = $state(false);

	onMount(async () => {
		if (!isDesktop) return;
		try {
			const info = await sendAndWait<{
				supported: boolean;
				mode: string | null;
				tray_available: boolean | null;
			}>(MessageType.GET_DISPLAY_MODE);
			// `tray_available === false` (not null) is the shell's explicit "no
			// StatusNotifierItem host will show the tray icon" verdict — null
			// means unreported, which must not nag.
			visible = info.supported && info.mode === 'browser' && info.tray_available === false;
		} catch {
			// The shell didn't answer: assume the tray is fine.
		}
	});

	function quit() {
		if (quitting) return;
		quitting = true;
		// Same path as the tray's Quit entry: the shell observes `shutdown` and
		// exits gracefully once the server drains. Fire-and-forget — the
		// process (and this socket) is about to go away.
		send(MessageType.SHUTDOWN);
	}
</script>

{#if visible}
	<div
		class="border-warning-500/40 bg-surface-900/80 flex items-center gap-3 border-b px-4 py-2"
		role="status"
		data-tray-unavailable-banner
	>
		<Icon icon="tabler:alert-triangle" class="text-warning-400 h-4 w-4 shrink-0" />
		<p class="text-surface-200 min-w-0 flex-1 text-xs">
			No system tray on this desktop — the app keeps running in the background after you close this
			tab.
		</p>
		<Button variant="danger" size="sm" onclick={quit} loading={quitting}>
			{quitting ? 'Quitting…' : 'Quit'}
		</Button>
	</div>
{/if}
