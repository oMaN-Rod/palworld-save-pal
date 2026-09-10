<script lang="ts">
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import type { GameStatusJson } from '$states/gameState.svelte';
	import { statusModeLabelKey } from './liveView.utils';

	let { status }: { status: GameStatusJson } = $props();

	const MODE_LABELS: Record<string, () => string> = {
		live_mode_solo_host: m.live_mode_solo_host,
		live_mode_dedicated: m.live_mode_dedicated,
		live_mode_client: m.live_mode_client
	};

	const modeLabel = $derived(MODE_LABELS[statusModeLabelKey(status.mode)]?.() ?? status.mode);

	const CHIP = 'rounded-sm px-2 py-0.5 text-xs font-medium whitespace-nowrap';
	const NEUTRAL = 'bg-surface-800 text-surface-300';
	const OK = 'bg-green-500/15 text-green-400';
	const WARN = 'bg-yellow-500/15 text-yellow-400';
</script>

<div id="live-status-chips" class="flex flex-wrap items-center gap-1.5">
	<span class={cn(CHIP, NEUTRAL, 'text-surface-400')}>v{status.modVersion}</span>
	<span class={cn(CHIP, NEUTRAL)}>{modeLabel}</span>
	<span class={cn(CHIP, status.authoritative ? OK : WARN)}>
		{status.authoritative ? m.live_status_host() : m.live_status_observer()}
	</span>
	<span class={cn(CHIP, status.worldLoaded ? OK : WARN)}>
		{status.worldLoaded ? m.live_chip_world_loaded() : m.live_chip_world_pending()}
	</span>
</div>
