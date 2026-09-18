<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { downloadKey, getModalState, getModsState, getNexusState, getServerState } from '$states';
	import { MessageType } from '$types';
	import type { LibraryMod, ModVersion, RecordedModError, VerificationStatus } from '$types';
	import * as m from '$i18n/messages';
	import ActionMenu, { menuItemClass } from './ActionMenu.svelte';
	import ModTile, { modTypeText } from './ModTile.svelte';
	import ModInUse from './ModInUse.svelte';
	import { targetName } from './targets';
	import { modTypeLabel as typeLabel } from './modLabels';
	import { verificationLabel } from './verificationText';
	import { iostoreErrorText } from './iostoreText';
	import { canConvertIostore } from './modCapabilities';
	import { updateAction } from './nexusUpdates';
	import { nexusErrorText } from './nexusText';
	import { modPageUrl } from './nexusSafety';
	import { openExternalLink } from '$lib/utils/externalLink';
	import type { ModsView } from './modsView.svelte';
	import {
		currentVersion,
		displayName,
		formatDate,
		formatSize,
		inUseLines,
		isClientOnlyOn,
		isSubscribed,
		refusedKind,
		totalSize
	} from './modList';

	let {
		mod,
		targetId,
		layout,
		selected = false,
		onDetails
	}: {
		mod: LibraryMod;
		targetId: string;
		layout: ModsView;
		selected?: boolean;
		onDetails: (modId: string) => void;
	} = $props();

	const modsState = getModsState();
	const nexusState = getNexusState();
	const modal = getModalState();
	const serverState = getServerState();
	const remoteMode = getRemoteMode();
	const mode = $derived({ desktop: PUBLIC_DESKTOP_MODE === 'true', remote: remoteMode.active });

	const update = $derived(nexusState.updateFor(mod.id));
	const action = $derived(updateAction(update, mode));
	const chainDownloading = $derived(
		update !== undefined && action.kind === 'update' && nexusState.downloadingFile(update.nexus_mod_id, action.fileId)
	);
	let chainApplying = $state(false);
	const chainPending = $derived(chainDownloading || chainApplying);
	let requestedNexusModId = $state<number | null>(null);
	const mineDownload = $derived(
		requestedNexusModId !== null && update?.nexus_mod_id === requestedNexusModId
	);
	/** Keyed off the update record's own `latest`, which survives it flipping to `up_to_date` on
	 *  install (`recordInstalled` spreads `...stored` and never clears `latest`) — so this chains
	 *  on any completed download of the file, not only one this row's own click started. */
	const chainKey = $derived(
		update?.latest ? downloadKey(update.nexus_mod_id, update.latest.file_id) : undefined
	);

	/** An outcome already present when this row is first shown is old news, not a fresh install to chain. */
	const announced = new Map<string, string | undefined>();

	$effect(() => {
		const key = chainKey;
		if (key === undefined) return;
		const installed = nexusState.lastInstalled[key];
		if (!announced.has(key)) {
			announced.set(key, installed?.version_id);
			return;
		}
		if (!installed || announced.get(key) === installed.version_id) return;
		announced.set(key, installed.version_id);
		untrack(() => {
			if (installed.version_id) {
				modsState.setCurrentVersion(installed.mod_id ?? mod.id, installed.version_id);
				modsState.apply(targetId);
				chainApplying = true;
			}
		});
	});

	/**
	 * Only the apply started while this chain is in flight resolves it, not a stale one.
	 *
	 * This still can't tell *this row's* apply from another apply on the same target (a manual
	 * Apply, or a second mod's chain): `profile_apply`'s `request_id` is generated fresh by the
	 * server on every call, including on an `apply_in_progress` refusal, and the request carries
	 * no client-supplied token the reply echoes back. `modsState.apply()` is fire-and-forget and
	 * returns nothing to correlate on. So this is a target-level signal, the same limit
	 * `ApplyBar.svelte` accepts for its own `lastApply`/`announced` tracking — in the rare case of
	 * two applies overlapping on one target, this row's pending line can clear on the other one's
	 * reply.
	 */
	const chainApplyAnnounced = new Map<string, string | undefined>();

	$effect(() => {
		if (!chainApplying) return;
		const last = modsState.lastApply[targetId];
		if (!chainApplyAnnounced.has(targetId)) {
			chainApplyAnnounced.set(targetId, last?.request_id);
			return;
		}
		if (!last || chainApplyAnnounced.get(targetId) === last.request_id) return;
		chainApplyAnnounced.set(targetId, last.request_id);
		untrack(() => {
			chainApplying = false;
		});
	});

	function requestUpdate(): void {
		if (action.kind !== 'update' || !update) return;
		requestedNexusModId = update.nexus_mod_id;
		modsState.clearLastError(MessageType.NEXUS_DOWNLOAD, targetId);
		nexusState.startDownload({
			target_id: targetId,
			mod_id: update.nexus_mod_id,
			file_id: action.fileId
		});
	}

	function updateDownloadLines(): string[] {
		if (!mineDownload) return [];
		const error = modsState.lastErrorFor(MessageType.NEXUS_DOWNLOAD, targetId);
		return error ? [nexusErrorText(error)] : [];
	}

	function updateIgnoreLines(): string[] {
		const error = modsState.lastErrorFor(MessageType.MOD_UPDATE_IGNORE, undefined, {
			mod_id: mod.id
		});
		return error ? [nexusErrorText(error)] : [];
	}

	const verificationPillClass: Record<VerificationStatus, string> = {
		verified: 'bg-success-500/20 text-success-400',
		missing: 'bg-error-500/20 text-error-400',
		unknown: 'bg-surface-700 text-surface-400',
		unexpected: 'bg-surface-700 text-surface-400'
	};

	const iostoreSuffix = '+iostore';

	const grid = $derived(layout === 'grid');
	const name = $derived(displayName(mod));
	const target = $derived(modsState.targets.find((entry) => entry.id === targetId));
	const profile = $derived(modsState.viewedProfile(targetId));
	const busy = $derived(modsState.settingMod[targetId] ?? false);
	const enabled = $derived(profile ? modsState.enabledIn(targetId, profile.id, mod.id) : false);
	const subscribed = $derived(isSubscribed(mod));
	const profileEntry = $derived(profile?.mods.find((entry) => entry.mod_id === mod.id));
	const pinnedId = $derived(profileEntry?.mod_version_id);
	const refused = $derived(target ? refusedKind(target, mod, pinnedId) : undefined);
	const blocked = $derived(!enabled && refused !== undefined);
	const version = $derived(currentVersion(mod));
	const pinned = $derived(
		pinnedId ? mod.versions.find((entry) => entry.id === pinnedId) : undefined
	);
	const applied = $derived(pinned ?? version);
	const targetVerification = $derived(modsState.verification[targetId]);
	const verification = $derived(enabled ? modsState.verificationFor(targetId, mod.id) : undefined);
	const iostoreBase = $derived(
		applied?.version.endsWith(iostoreSuffix)
			? applied.version.slice(0, -iostoreSuffix.length)
			: undefined
	);
	/** Only when a current version exists that is neither the converted build nor its base. */
	const iostoreNote = $derived(
		iostoreBase !== undefined &&
			version &&
			version.version !== applied?.version &&
			version.version !== iostoreBase
			? iostoreBase
			: undefined
	);
	/** `mod_iostore_convert` runs against the active profile, not the one being viewed. */
	const activeEntry = $derived(
		modsState.activeProfile(targetId)?.mods.find((entry) => entry.mod_id === mod.id)
	);
	const convertible = $derived.by(() => {
		if (!target || !activeEntry) return false;
		const active: ModVersion | undefined = activeEntry.mod_version_id
			? mod.versions.find((entry) => entry.id === activeEntry.mod_version_id)
			: version;
		return canConvertIostore(target, mode, mod, active);
	});
	const removeError = $derived(
		modsState.lastErrorFor(MessageType.MOD_REMOVE, undefined, { mod_id: mod.id })
	);
	const inUse = $derived(
		removeError?.code === 'version_in_use' && Array.isArray(removeError.holders)
			? removeError
			: undefined
	);
	const lines = $derived([
		...setModLines(),
		...profileRemoveLines(),
		...removeLines(),
		...versionLines(),
		...iostoreLines(),
		...updateDownloadLines(),
		...updateIgnoreLines()
	]);
	const clientOnly = $derived(target ? isClientOnlyOn(target, mod) : false);

	const lockId = $derived(`mod-lock-${targetId}-${mod.id}`);

	const usageLabels = {
		target(id: string): string {
			const found = modsState.targets.find((entry) => entry.id === id);
			return found ? targetName(found, serverState.servers) : id;
		},
		profile(id: string): string {
			return (
				Object.values(modsState.profiles)
					.flat()
					.find((entry) => entry.id === id)?.name ?? id
			);
		}
	};

	function refusedText(kind: string): string {
		if (kind === 'workshop' && target?.kind === 'server') return m.mods_list_docker_workshop();
		if (kind === 'nativedll' && target?.kind === 'client') return m.mods_list_native_client();
		return m.mods_list_not_supported({ kind: typeLabel(kind) });
	}

	function setModLines(): string[] {
		const error = modsState.lastErrorFor(MessageType.PROFILE_SET_MOD, targetId, { mod_id: mod.id });
		if (!error) return [];
		if (error.code === 'not_subscribed_on_target') return [m.mods_panel_error_not_subscribed()];
		if (error.code === 'not_supported_on_target') return [refusedText(String(error.kind ?? ''))];
		return [error.message];
	}

	function removeLines(): string[] {
		const error = removeError;
		if (!error) return [];
		if (error.code === 'version_in_use') {
			return Array.isArray(error.holders) ? [] : inUseLines(error, usageLabels);
		}
		if (error.code === 'apply_in_progress') {
			return [
				m.mods_list_remove_busy({ target: usageLabels.target(String(error.target_id ?? '')) })
			];
		}
		return [error.message];
	}

	function profileRemoveLines(): string[] {
		const error = modsState.lastErrorFor(MessageType.PROFILE_REMOVE_MOD, targetId, {
			mod_id: mod.id
		});
		if (!error) return [];
		return error.code === 'mod_not_in_profile'
			? [m.mods_list_remove_from_profile_missing()]
			: [error.message];
	}

	function versionErrorLines(error: RecordedModError): string[] {
		return error.code === 'version_in_use' ? inUseLines(error, usageLabels) : [error.message];
	}

	function versionLines(): string[] {
		const setCurrent = modsState.lastErrorFor(MessageType.MOD_VERSION_SET_CURRENT, undefined, {
			mod_id: mod.id
		});
		const deleted = mod.versions
			.map((entry) =>
				modsState.lastErrorFor(MessageType.MOD_VERSION_DELETE, undefined, { version_id: entry.id })
			)
			.find((error) => error !== undefined);
		return [
			...(setCurrent ? versionErrorLines(setCurrent) : []),
			...(deleted ? versionErrorLines(deleted) : [])
		];
	}

	function iostoreLines(): string[] {
		const error = modsState.lastErrorFor(MessageType.MOD_IOSTORE_CONVERT, targetId, {
			mod_id: mod.id
		});
		return error ? [iostoreErrorText(error)] : [];
	}

	async function removeMod() {
		const confirmed = await modal.showConfirmModal({
			title: m.mods_list_remove(),
			message: m.mods_list_remove_message({ name }),
			confirmText: m.mods_list_remove(),
			cancelText: m.mods_panel_cancel()
		});
		if (confirmed) modsState.removeMod(mod.id);
	}
</script>

{#snippet pills()}
	{#if verification}
		<span
			class={[
				'rounded-xs px-1.5 py-0.5 text-[10px] font-medium',
				verificationPillClass[verification.status],
				targetVerification?.live === false && 'opacity-60'
			]}
			title={targetVerification?.live === false
				? m.mods_verify_last_seen({ time: formatDate(targetVerification.checked_at) })
				: undefined}
		>
			{verificationLabel(verification.status)}
		</span>
	{/if}
	{#if iostoreBase !== undefined}
		<span class="rounded-xs bg-cyan-500/15 px-1.5 py-0.5 text-[10px] font-medium text-cyan-400">
			{m.mods_iostore_pill()}
		</span>
	{/if}
	{#if action.kind === 'update'}
		<span class="rounded-xs bg-amber-500/15 px-1.5 py-0.5 text-[10px] font-medium text-amber-400">
			{m.mods_update_available({ version: action.version })}
		</span>
	{/if}
{/snippet}

{#snippet controls()}
	<div class="flex justify-between w-full items-center">
		<button
			role="switch"
			aria-checked={enabled}
			aria-label={name}
			aria-describedby={blocked ? lockId : undefined}
			class={[
				'relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-50',
				enabled ? 'bg-success-500' : 'bg-surface-700'
			]}
			disabled={busy || !profile || blocked}
			onclick={() => profile && modsState.setMod(targetId, mod.id, !enabled, profile.id)}
		>
			<span
				class={[
					'absolute top-0.5 size-4 rounded-full transition-all',
					enabled ? 'bg-surface-50 left-[18px]' : 'bg-surface-300 left-0.5'
				]}
			></span>
		</button>
		<ActionMenu label={m.mods_list_actions({ name })}>
			{#snippet items({ close })}
				<button
					role="menuitem"
					class={menuItemClass}
					onclick={() => {
						close();
						onDetails(mod.id);
					}}
				>
					<Icon icon="tabler:info-circle" size={14} />
					{m.mods_list_details()}
				</button>
				{#if update && (action.kind === 'update' || action.kind === 'ignored')}
					<a
						role="menuitem"
						class={menuItemClass}
						href={modPageUrl(update.nexus_mod_id, action.kind === 'update' ? action.fileId : undefined)}
						target="_blank"
						rel="noreferrer"
						onclick={(event: MouseEvent) => {
							close();
							openExternalLink(
								event,
								modPageUrl(update.nexus_mod_id, action.kind === 'update' ? action.fileId : undefined)
							);
						}}
					>
						<Icon icon="tabler:external-link" size={14} />
						{m.mods_update_open_page({ name })}
					</a>
				{/if}
				{#if action.kind === 'update'}
					<button
						role="menuitem"
						class={menuItemClass}
						disabled={chainPending}
						onclick={() => {
							close();
							requestUpdate();
						}}
					>
						<Icon icon="tabler:download" size={14} />
						{m.mods_update_action({ name, version: action.version })}
					</button>
					<button
						role="menuitem"
						class={menuItemClass}
						onclick={() => {
							close();
							nexusState.ignoreVersion(mod.id, action.version);
						}}
					>
						<Icon icon="tabler:eye-off" size={14} />
						{m.mods_update_ignore({ version: action.version, name })}
					</button>
				{:else if action.kind === 'ignored'}
					<button
						role="menuitem"
						class={menuItemClass}
						onclick={() => {
							close();
							nexusState.ignoreVersion(mod.id, null);
						}}
					>
						<Icon icon="tabler:eye" size={14} />
						{m.mods_update_unignore({ name })}
					</button>
				{/if}
				{#if convertible}
					<button
						role="menuitem"
						class={menuItemClass}
						disabled={modsState.converting[targetId] ?? false}
						onclick={() => {
							close();
							modsState.convertIostore(targetId, mod.id);
						}}
					>
						<Icon icon="tabler:transform" size={14} />
						{m.mods_iostore_convert()}
					</button>
				{/if}
				{#if profile && profileEntry}
					<button
						role="menuitem"
						class={menuItemClass}
						disabled={busy}
						onclick={() => {
							close();
							modsState.removeFromProfile(targetId, mod.id, profile.id);
						}}
					>
						<Icon icon="tabler:playlist-x" size={14} />
						{m.mods_list_remove_from_profile()}
					</button>
				{/if}
				<button
					role="menuitem"
					class={[menuItemClass, 'text-red-400']}
					onclick={() => {
						close();
						removeMod();
					}}
				>
					<Icon icon="tabler:trash-x" size={14} />
					{m.mods_list_remove()}
				</button>
			{/snippet}
		</ActionMenu>
	</div>
{/snippet}

{#snippet notes()}
	{#if (blocked && refused) || clientOnly || iostoreNote !== undefined || lines.length > 0 || inUse || action.kind === 'ignored' || chainPending}
		<div class={['flex flex-col gap-1 text-xs', grid ? 'px-3 pt-1.5' : 'pl-15']}>
			{#if blocked && refused}
				<p id={lockId} class="text-surface-400 flex items-start gap-1.5">
					<Icon icon="tabler:lock" size={12} class="mt-0.5 shrink-0" />
					{refusedText(refused)}
				</p>
			{/if}
			{#if clientOnly}
				<p class="text-warning-400 flex items-start gap-1.5">
					<Icon icon="tabler:alert-triangle" size={12} class="mt-0.5 shrink-0" />
					{m.mods_list_client_only_server()}
				</p>
			{/if}
			{#if iostoreNote !== undefined}
				<p class="text-surface-400 flex items-start gap-1.5">
					<Icon icon="tabler:info-circle" size={12} class="mt-0.5 shrink-0" />
					{m.mods_iostore_newer({ version: iostoreNote })}
				</p>
			{/if}
			{#if action.kind === 'ignored'}
				<p class="text-surface-400 flex items-start gap-1.5">
					<Icon icon="tabler:eye-off" size={12} class="mt-0.5 shrink-0" />
					{m.mods_update_ignored({ version: action.version })}
				</p>
			{/if}
			{#if chainPending}
				<p class="text-surface-400 flex items-start gap-1.5">
					<Icon icon="tabler:loader-2" size={12} class="mt-0.5 shrink-0 animate-spin" />
					{m.mods_update_applying({ name })}
				</p>
			{/if}
			{#each lines as line}
				<p class="text-error-400 flex items-start gap-1.5" role="alert">
					<Icon icon="tabler:alert-circle" size={12} class="mt-0.5 shrink-0" />
					{line}
				</p>
			{/each}
			{#if inUse}
				<ModInUse {mod} error={inUse} labels={usageLabels} onRetry={removeMod} />
			{/if}
		</div>
	{/if}
{/snippet}

<div
	data-mod-id={mod.id}
	class={[
		'bg-surface-900 flex overflow-hidden rounded-lg border transition-colors',
		grid ? 'flex-col' : 'flex-col gap-1.5 p-2',
		selected
			? 'border-primary-400 ring-primary-400 ring-1'
			: lines.length > 0 || inUse !== undefined
				? 'border-error-500/50'
				: 'border-surface-800 hover:border-surface-600'
	]}
>
	<div class={grid ? 'contents' : 'flex items-center gap-3'}>
		<button
			class={[
				'focus-visible:outline-primary-300 min-w-0 cursor-pointer text-left focus-visible:outline-2 focus-visible:-outline-offset-2',
				grid ? 'flex flex-col' : 'flex flex-1 items-center gap-3'
			]}
			aria-label={m.mods_card_open({ name })}
			aria-expanded={selected}
			onclick={() => onDetails(mod.id)}
		>
			<ModTile
				{mod}
				muted={!enabled}
				class={grid ? 'aspect-video w-full text-4xl' : 'size-12 shrink-0 rounded-md text-base'}
			>
				{#if grid}
					<span
						class={[
							'bg-surface-950/80 absolute top-2 left-2 rounded-xs px-1.5 py-0.5 text-[10px] font-bold tracking-wide uppercase backdrop-blur-sm',
							modTypeText[mod.mod_type] ?? 'text-surface-300'
						]}
					>
						{typeLabel(mod.mod_type)}
					</span>
					<span class="absolute bottom-2 left-2 flex gap-1">{@render pills()}</span>
				{/if}
				{#if blocked}
					<span
						class="bg-surface-950/60 text-surface-300 absolute inset-0 flex items-center justify-center"
					>
						<Icon icon="tabler:lock" size={grid ? 24 : 16} />
					</span>
				{/if}
			</ModTile>
			<span class={['flex min-w-0 flex-col gap-0.5', grid && 'px-3 pt-2.5']}>
				<span class={['text-sm font-medium', grid ? 'line-clamp-2' : 'truncate']}>{name}</span>
				<span class="text-surface-400 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs">
					{#if !grid}
						<span class={modTypeText[mod.mod_type] ?? 'text-surface-400'}>
							{typeLabel(mod.mod_type)}
						</span>
					{/if}
					{#if mod.author}
						<span>{m.mods_panel_by_author({ author: mod.author })}</span>
					{/if}
					{#if version}
						<span>{m.mods_review_version({ version: version.version })}</span>
					{/if}
					<span>{formatSize(totalSize(mod))}</span>
					{#if !grid}
						{@render pills()}
					{/if}
					{#if subscribed}
						<span class="flex items-center gap-1 text-cyan-400">
							<Icon icon="tabler:brand-steam" size={11} />
							{m.mods_list_steam_workshop()}
						</span>
					{/if}
				</span>
				{#if grid}
					<span
						class={[
							'flex min-w-0 flex-1 items-center gap-2 text-xs',
							enabled ? 'text-surface-100' : 'text-surface-400'
						]}
					>
						{#if pinned}
							<span
								class="text-primary-300 flex items-center gap-0.5"
								title={m.mods_pin_version({ version: pinned.version })}
							>
								<Icon icon="tabler:pin" size={12} />
								{pinned.version}
							</span>
						{/if}
					</span>
				{/if}
			</span>
		</button>
		{#if !grid}
			<div class="flex shrink-0 items-center gap-1">{@render controls()}</div>
		{/if}
	</div>
	{@render notes()}
	{#if grid}
		<div class="mt-auto flex items-center gap-2 pt-2 pr-1.5 pb-2 pl-3">{@render controls()}</div>
	{/if}
</div>
