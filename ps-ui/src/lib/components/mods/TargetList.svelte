<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Component } from 'svelte';
	import { Button, Card, Tooltip } from '$components/ui';
	import { getModalState, getModsState, getServerState } from '$states';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { isWebBuild } from '$lib/utils/platform';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import AddTargetModal from './AddTargetModal.svelte';
	import {
		confirmRemoveTarget,
		hazardText,
		platformLabel,
		targetHazards,
		targetName
	} from './targets';

	let { selectedId, onselect }: { selectedId?: string; onselect: (targetId: string) => void } =
		$props();

	const modsState = getModsState();
	const modal = getModalState();
	const serverState = getServerState();
	const remoteMode = getRemoteMode();

	const canAdd = $derived(!isWebBuild && !remoteMode.active);

	async function openAdd() {
		const added = await modal.showModal<string | null>(AddTargetModal as unknown as Component, {});
		if (added) onselect(added);
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between">
		<h2 class="heading-gradient text-xl font-bold">{m.mods_panel_title()}</h2>
		{#if canAdd}
			<Button variant="primary" size="sm" class="flex items-center gap-2" onclick={openAdd}>
				<Icon icon="tabler:plus" size={14} />
				{m.mods_add_target()}
			</Button>
		{/if}
	</div>

	{#if modsState.targets.length === 0}
		<Card class="text-surface-400 text-center">
			<Icon icon="tabler:device-gamepad-2" size={32} class="mx-auto mb-2 opacity-50" />
			<p>{m.mods_targets_empty()}</p>
			<p class="mt-1 text-sm">
				{remoteMode.active ? m.mods_targets_empty_hint_remote() : m.mods_targets_empty_hint()}
			</p>
		</Card>
	{:else}
		<div class="flex flex-col gap-2">
			{#each modsState.targets as target (target.id)}
				{@const name = targetName(target, serverState.servers)}
				{@const hazards = targetHazards(target)}
				{@const idPrefix = `mods-target-${target.id}`}
				{@const describedBy = [
					`${idPrefix}-kind`,
					`${idPrefix}-platform`,
					`${idPrefix}-path`,
					...(hazards.length > 0 ? [`${idPrefix}-hazards`] : [])
				].join(' ')}
				<div
					data-target-id={target.id}
					class={cn(
						'relative rounded-sm transition-all',
						selectedId === target.id ? 'ring-secondary-500 ring-2' : ''
					)}
				>
					<Card class="hover:bg-surface-800 flex items-start gap-2" padding="p-3">
						<button
							class="focus-visible:outline-primary-300 absolute inset-0 cursor-pointer rounded-sm focus-visible:outline-2 focus-visible:outline-offset-2"
							aria-label={name}
							aria-describedby={describedBy}
							aria-current={selectedId === target.id ? 'true' : undefined}
							onclick={() => onselect(target.id)}
						></button>
						<div class="pointer-events-none relative flex min-w-0 flex-1 flex-col gap-1">
							<div class="flex items-center gap-2">
								<span class="truncate font-bold">{name}</span>
								<span
									id={`${idPrefix}-kind`}
									class={cn(
										'shrink-0 rounded-sm px-1.5 py-0.5 text-[10px] font-medium uppercase',
										target.kind === 'server'
											? 'bg-cyan-500/15 text-cyan-400'
											: 'bg-blue-500/15 text-blue-400'
									)}
								>
									{target.kind === 'server'
										? m.mods_target_kind_server()
										: m.mods_target_kind_client()}
								</span>
								{#if hazards.length > 0}
									<Tooltip
										label={hazards.map(hazardText).join(' ')}
										baseClass="pointer-events-auto flex shrink-0"
									>
										<Icon
											icon="tabler:alert-triangle"
											size={14}
											class="text-warning-400"
											role="img"
											aria-label={m.mods_hazard_label()}
										/>
									</Tooltip>
									<span id={`${idPrefix}-hazards`} hidden>{hazards.map(hazardText).join(' ')}</span>
								{/if}
							</div>
							<span id={`${idPrefix}-platform`} class="text-surface-400 text-xs">
								{platformLabel(target.platform)}
							</span>
							<Tooltip
								label={target.root_path}
								baseClass="pointer-events-auto min-w-0 max-w-full self-start"
							>
								<span
									id={`${idPrefix}-path`}
									class="text-surface-400 block truncate font-mono text-xs"
								>
									{target.root_path}
								</span>
							</Tooltip>
						</div>
						{#if target.kind === 'client'}
							<Button
								variant="ghost"
								size="icon"
								class="relative"
								aria-label={m.mods_target_remove()}
								onclick={() => confirmRemoveTarget(target, name, modal, modsState)}
							>
								<Icon icon="tabler:trash-x" size={14} class="text-red-400" />
							</Button>
						{/if}
					</Card>
				</div>
			{/each}
		</div>
	{/if}
</div>
