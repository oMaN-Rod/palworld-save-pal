<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { Card } from '$components/ui';
	import { GameCommandError, getGameState, getModsState } from '$states';
	import type { GameInstanceJson } from '$lib/states/gameState.svelte';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { canBindInstances } from './modCapabilities';
	import { instanceErrorText } from './verificationText';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const gameState = getGameState();
	const remoteMode = getRemoteMode();
	const baseId = $props.id();

	const mode = $derived({ desktop: PUBLIC_DESKTOP_MODE === 'true', remote: remoteMode.active });
	const bindable = $derived(canBindInstances(mode));
	const verification = $derived(modsState.verification[targetId]);
	const unexpected = $derived(
		verification?.status.filter((entry) => entry.status === 'unexpected') ?? []
	);
	const buildInfoLine = $derived(
		verification?.build_info ? formatBuildInfo(verification.build_info) : ''
	);
	const verificationError = $derived(
		modsState.lastErrorFor(MessageType.MOD_VERIFICATION_GET, targetId)
	);

	let bindErrors = $state<Record<string, string>>({});
	let binding = $state<Record<string, boolean>>({});

	$effect(() => {
		const id = targetId;
		void modsState.resets;
		untrack(() => {
			modsState.loadVerification(id);
			if (bindable) gameState.refreshInstances();
		});
	});

	function formatTime(iso: string): string {
		const date = new Date(iso);
		return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
	}

	function clientTargetName(id: string): string {
		return modsState.targets.find((entry) => entry.id === id)?.name ?? id;
	}

	function otherClientTargets(): { id: string; name: string }[] {
		return modsState.targets
			.filter((entry) => entry.kind === 'client' && entry.id !== targetId)
			.map((entry) => ({ id: entry.id, name: entry.name }));
	}

	function unlistedBinding(instance: GameInstanceJson): { id: string; label: string } | undefined {
		const id = instance.targetId;
		if (!id || id === targetId) return undefined;
		if (otherClientTargets().some((entry) => entry.id === id)) return undefined;
		const target = modsState.targets.find((entry) => entry.id === id);
		return {
			id,
			label: target ? m.mods_live_bind_other({ name: target.name }) : m.mods_live_bind_unknown()
		};
	}

	function formatBuildInfo(info: Record<string, string>): string {
		const parts: string[] = [];
		if (info.ue4ssVersion) parts.push(m.mods_live_build_ue4ss({ version: info.ue4ssVersion }));
		if (info.amityVersion) parts.push(m.mods_live_build_amity({ version: info.amityVersion }));
		if (info.platform) parts.push(info.platform);
		return parts.join(', ');
	}

	async function rebind(instanceId: string, select: HTMLSelectElement) {
		const value = select.value;
		binding[instanceId] = true;
		try {
			await gameState.setInstanceTarget(instanceId, value === '' ? null : value);
			delete bindErrors[instanceId];
			bindErrors = { ...bindErrors };
		} catch (error) {
			const code = error instanceof GameCommandError ? error.code : undefined;
			const message = error instanceof Error ? error.message : String(error);
			bindErrors = { ...bindErrors, [instanceId]: instanceErrorText(code, message) };
		} finally {
			delete binding[instanceId];
			binding = { ...binding };
		}
		select.value = gameState.instances.find((entry) => entry.id === instanceId)?.targetId ?? '';
	}
</script>

<div class="flex flex-col gap-4">
	{#if verificationError}
		<p class="text-error-400 text-sm" role="alert">{verificationError.message}</p>
	{/if}

	{#if verification}
		<Card padding="p-3" class="flex flex-col gap-2 text-sm">
			<p class="flex items-center gap-2 font-medium">
				<Icon icon={verification.live ? 'tabler:plug-connected' : 'tabler:plug-off'} size={16} />
				{verification.live
					? m.mods_live_connected()
					: m.mods_live_last_seen({ time: formatTime(verification.checked_at) })}
			</p>
			{#if buildInfoLine}
				<p class="text-surface-400 text-xs">{buildInfoLine}</p>
			{/if}
			{#if !verification.resolution.complete}
				<p class="text-warning-400 flex items-center gap-2 text-xs">
					<Icon icon="tabler:alert-triangle" size={12} class="shrink-0" />
					{m.mods_live_signatures_missing({ names: verification.resolution.missing.join(', ') })}
				</p>
			{/if}
		</Card>

		{#if unexpected.length > 0}
			<div class="flex flex-col gap-2">
				<h3 class="text-surface-300 text-xs font-medium uppercase">
					{m.mods_live_unexpected_title()}
				</h3>
				<ul class="flex flex-col gap-1 text-sm">
					{#each unexpected as entry, index (index)}
						<li class="text-surface-400">{entry.name}</li>
					{/each}
				</ul>
			</div>
		{/if}
	{:else}
		<Card padding="p-6" class="text-surface-400 flex items-center gap-3 text-sm">
			<Icon icon="tabler:plug-off" size={20} class="shrink-0" />
			{m.mods_live_none()}
		</Card>
	{/if}

	<div class="flex flex-col gap-2">
		<h3 class="text-surface-300 text-xs font-medium uppercase">
			{m.mods_live_connections_title()}
		</h3>
		{#if !bindable}
			<p class="text-surface-400 text-sm">{m.mods_live_connections_remote()}</p>
		{/if}
		{#if bindable}
			{#each gameState.instances as instance (instance.id)}
				{@const selectId = `${baseId}-instance-${instance.id}`}
				<div data-instance-id={instance.id}>
					<Card padding="p-3" class="flex flex-col gap-2">
						<div class="flex flex-wrap items-center justify-between gap-3">
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">{instance.name}</p>
								<p class="text-surface-400 text-xs">
									{instance.live ? m.mods_live_instance_live() : m.mods_live_instance_offline()}
								</p>
							</div>
							{#if instance.source === 'saved' && bindable}
								{@const extra = unlistedBinding(instance)}
								<select
									id={selectId}
									aria-label={m.mods_live_bind_label({ name: instance.name })}
									class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-1.5 text-sm focus:outline-none disabled:opacity-60"
									value={instance.targetId ?? ''}
									disabled={binding[instance.id]}
									onchange={(event) => rebind(instance.id, event.currentTarget)}
								>
									<option value="">{m.mods_live_bind_none()}</option>
									<option value={targetId}>{m.mods_live_bind_this()}</option>
									{#each otherClientTargets() as entry (entry.id)}
										<option value={entry.id}>{entry.name}</option>
									{/each}
									{#if extra}
										<option value={extra.id}>{extra.label}</option>
									{/if}
								</select>
							{:else if instance.source === 'auto'}
								<p class="text-surface-400 text-xs">
									{#if instance.targetId === targetId}
										{m.mods_live_auto_bound()}
									{:else if instance.targetId}
										{m.mods_live_auto_other({ name: clientTargetName(instance.targetId) })}
									{:else}
										{m.mods_live_auto_unbound()}
									{/if}
								</p>
							{/if}
						</div>
						{#if bindErrors[instance.id]}
							<p class="text-error-400 text-xs" role="alert">{bindErrors[instance.id]}</p>
						{/if}
					</Card>
				</div>
			{/each}
		{/if}
	</div>
</div>
