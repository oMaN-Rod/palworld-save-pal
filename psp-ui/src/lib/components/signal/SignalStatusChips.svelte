<script lang="ts">
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import type { SignalSourceHealth, SignalStatusJson } from '$types';
	import { sourceKindFromStatus } from './signalSource.utils';

	let { status }: { status: SignalStatusJson } = $props();

	const HEALTH_LABELS: Record<SignalSourceHealth, () => string> = {
		idle: m.signal_health_idle,
		waiting: m.signal_health_waiting,
		auth: m.signal_health_auth,
		down: m.signal_health_down,
		stale: m.signal_health_stale,
		ok: m.signal_health_ok
	};

	const HEALTH_COLOR: Record<SignalSourceHealth, string> = {
		idle: 'bg-surface-500/20 text-surface-300',
		waiting: 'bg-blue-500/15 text-blue-400',
		auth: 'bg-orange-500/15 text-orange-400',
		down: 'bg-red-500/15 text-red-400',
		stale: 'bg-yellow-500/15 text-yellow-400',
		ok: 'bg-green-500/15 text-green-400'
	};

	const SOURCE_LABELS: Record<string, () => string> = {
		off: m.signal_source_off,
		file: m.signal_source_file,
		server: m.signal_source_server
	};

	const sourceKind = $derived(sourceKindFromStatus(status.source.kind));
	const source = $derived(status.source);

	const CHIP = 'rounded-sm px-2 py-0.5 text-xs font-medium whitespace-nowrap';
	const NEUTRAL = 'bg-surface-800 text-surface-300';
</script>

<div id="signal-status-chips" class="flex flex-wrap items-center gap-1.5">
	<span class={cn(CHIP, NEUTRAL)}>{SOURCE_LABELS[sourceKind]?.()}</span>

	{#if sourceKind !== 'off'}
		<span class={cn(CHIP, HEALTH_COLOR[source.health])}>{HEALTH_LABELS[source.health]()}</span>
		<span class={cn(CHIP, NEUTRAL, 'text-surface-400')}>
			{m.signal_source_actors({ count: source.actorCount })}
		</span>
	{/if}

	<span
		class={cn(
			CHIP,
			status.armed ? 'bg-green-500/15 text-green-400' : 'bg-yellow-500/15 text-yellow-400'
		)}
	>
		{status.armed ? m.signal_remote_access_armed_indicator() : m.signal_chip_not_armed()}
	</span>
</div>
