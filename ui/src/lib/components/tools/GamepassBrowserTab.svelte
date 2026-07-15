<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getToastState } from '$states';
	import { Card, Spinner } from '$components/ui';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType, type GamepassSave } from '$types';
	import { GamepassBrowser } from '$components/gamepass';
	import * as m from '$i18n/messages';
	import { Monitor, RefreshCw } from 'lucide-svelte';

	// `active` is true while this tab is the selected one. Skeleton keeps every
	// panel mounted, so the first scan is gated on activation to preserve the
	// original "scan once, on first view" behavior rather than scanning on load.
	let { active = false }: { active?: boolean } = $props();

	const toast = getToastState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let browserSaves: Record<string, GamepassSave> = $state({});
	let isBrowserScanning = $state(false);
	let browserLoaded = $state(false);

	async function loadBrowserSaves() {
		isBrowserScanning = true;
		try {
			const result = await sendAndWait<{
				saves?: Record<string, GamepassSave>;
				error?: string;
			}>(MessageType.SCAN_GAMEPASS_SAVES, {});
			if (result.saves) {
				browserSaves = result.saves;
				browserLoaded = true;
			}
		} catch (err: any) {
			toast.add(m.tools_scan_failed({ error: err.message }), m.error(), 'error');
		} finally {
			isBrowserScanning = false;
		}
	}

	$effect(() => {
		if (active && !browserLoaded && isDesktopMode) {
			loadBrowserSaves();
		}
	});
</script>

<div class="flex flex-col gap-6">
	{#if !isDesktopMode}
		<Card class="mx-auto max-w-lg">
			<div class="flex flex-col items-center gap-4 p-4">
				<Monitor size={48} class="text-surface-400" />
				<p class="text-surface-300 text-center">{m.tools_gamepass_desktop_required()}</p>
			</div>
		</Card>
	{:else if isBrowserScanning}
		<div class="flex flex-col items-center gap-4">
			<Spinner />
			<span class="text-surface-200">{m.tools_scanning_gamepass()}</span>
		</div>
	{:else}
		<div class="flex items-center justify-between">
			<p class="text-surface-400 text-sm">{m.tools_gamepass_browse_hint()}</p>
			<button
				class="text-surface-400 hover:text-surface-200 flex items-center gap-1.5 text-sm"
				onclick={loadBrowserSaves}
			>
				<RefreshCw size={14} />
				{m.tools_refresh()}
			</button>
		</div>
		<GamepassBrowser bind:saves={browserSaves} manageable={true} onchange={loadBrowserSaves} />
	{/if}
</div>
