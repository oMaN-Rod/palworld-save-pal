<script lang="ts">
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import type { GameInstanceJson } from '$states/gameState.svelte';

	let {
		instances,
		activeId,
		activeConnected = false,
		pending = false,
		onselect,
		onedit,
		onadd
	}: {
		instances: GameInstanceJson[];
		activeId: string | null;
		activeConnected?: boolean;
		pending?: boolean;
		onselect: (id: string) => void;
		onedit: (instance: GameInstanceJson) => void;
		onadd: () => void;
	} = $props();

	const ROW = 'flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-xs';
	const ACTIVE = 'bg-surface-700 text-surface-50';
	const IDLE = 'text-surface-300 hover:bg-surface-800';

	function dotClass(instance: GameInstanceJson): string {
		if (instance.source === 'auto') return 'bg-green-400';
		if (instance.id === activeId) return activeConnected ? 'bg-green-400' : 'bg-red-400';
		return 'border border-surface-500';
	}
</script>

<div id="live-instance-switcher" class="flex flex-col gap-1">
	{#if instances.length === 0}
		<p class="text-surface-400 px-2 py-1.5 text-xs">{m.live_instance_none()}</p>
	{:else}
		{#each instances as instance (instance.id)}
			<div class="flex items-center gap-1">
				<button
					type="button"
					data-instance-id={instance.id}
					aria-current={instance.id === activeId ? 'true' : 'false'}
					disabled={pending}
					class={cn(ROW, instance.id === activeId ? ACTIVE : IDLE)}
					onclick={() => onselect(instance.id)}
				>
					<span class={cn('size-1.5 shrink-0 rounded-full', dotClass(instance))}></span>
					<span class="grow truncate">{instance.name}</span>
					<span class="text-surface-400 shrink-0">{instance.host}:{instance.port}</span>
					<span class="text-surface-500 shrink-0">
						{instance.source === 'auto' ? m.live_instance_auto() : m.live_instance_saved()}
					</span>
				</button>
				{#if instance.source === 'saved'}
					<button
						type="button"
						data-edit-id={instance.id}
						class="text-surface-400 hover:text-surface-100 rounded-sm px-1.5 py-1 text-xs"
						onclick={() => onedit(instance)}
					>
						{m.live_instance_edit()}
					</button>
				{/if}
			</div>
		{/each}
	{/if}

	<button
		type="button"
		data-action="add-instance"
		class="text-surface-300 hover:bg-surface-800 rounded-sm px-2 py-1.5 text-left text-xs"
		onclick={onadd}
	>
		{m.live_instance_add()}
	</button>
</div>
