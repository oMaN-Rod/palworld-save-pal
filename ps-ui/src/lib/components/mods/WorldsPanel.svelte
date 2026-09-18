<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { onMount } from 'svelte';
	import { Button, Card } from '$components/ui';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import type { LocalSave } from '$types';
	import * as m from '$i18n/messages';
	import { sortByRecency } from '$lib/components/signal/pickerModel';
	import { worldLinkErrorText } from './profileText';

	let { targetId, canLaunch }: { targetId: string; canLaunch: boolean } = $props();

	const modsState = getModsState();
	const baseId = $props.id();

	const profiles = $derived(modsState.profiles[targetId] ?? []);
	const saves = $derived(sortByRecency(modsState.localSaves ?? [], (save) => save.modified_ms));
	const listError = $derived(modsState.lastErrorFor(MessageType.LIST_LOCAL_SAVES));
	const linkError = $derived(modsState.lastErrorFor(MessageType.WORLD_PROFILE_SET));
	const launching = $derived(modsState.launching[targetId] ?? false);

	onMount(() => modsState.loadLocalSaves());

	function linkedElsewhere(save: LocalSave): boolean {
		return save.mod_profile !== null && save.mod_profile.target_id !== targetId;
	}

	/** A one-way `value` never re-selects on its own, so the control goes back to the stored link until a reply changes it. */
	function relink(save: LocalSave, select: HTMLSelectElement) {
		const value = select.value;
		modsState.setWorldProfile(save.world_key, save.name, value === '' ? null : value);
		select.value = save.mod_profile?.profile_id ?? '';
	}

	function formatDate(ms: number): string {
		return ms > 0 ? new Date(ms).toLocaleString() : '';
	}
</script>

<div class="flex flex-col gap-3">
	<div class="flex items-start justify-between gap-3">
		<p class="text-surface-400 text-xs">{m.mods_worlds_hint()}</p>
		<Button
			variant="ghost"
			size="sm"
			disabled={modsState.loadingSaves}
			onclick={() => modsState.loadLocalSaves()}
		>
			<Icon icon="tabler:refresh" size={14} />
			{m.mods_worlds_refresh()}
		</Button>
	</div>

	{#if listError}
		<p class="text-error-400 text-sm" role="alert">
			{m.mods_worlds_list_failed({ message: listError.message })}
		</p>
	{/if}

	{#if modsState.loadingSaves && modsState.localSaves === undefined}
		<p class="text-surface-400 text-sm" role="status">{m.mods_worlds_loading()}</p>
	{:else if modsState.localSaves !== undefined && saves.length === 0}
		<Card class="text-surface-400 text-center text-sm">{m.mods_worlds_empty()}</Card>
	{/if}

	{#each saves as save, index (save.world_key)}
		{@const rowId = `${baseId}-world-${index}`}
		{@const modified = formatDate(save.modified_ms)}
		{@const gamepass = save.save_type === 'gamepass'}
		<div data-world-key={save.world_key}>
			<Card padding="p-3" class="flex flex-col gap-2">
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div class="min-w-0">
						<p id={`${rowId}-name`} class="truncate text-sm font-medium">
							{save.name}
							{#if gamepass}
								<span
									aria-hidden="true"
									class="rounded-sm bg-cyan-500/15 px-1.5 py-0.5 text-[10px] font-medium text-cyan-400"
								>
									{m.mods_worlds_gamepass()}
								</span>
							{/if}
						</p>
						{#if gamepass}
							<span id={`${rowId}-platform`} class="sr-only">{m.mods_worlds_gamepass_suffix()}</span
							>
						{/if}
						{#if modified}
							<p class="text-surface-500 text-xs">{m.mods_worlds_modified({ date: modified })}</p>
						{/if}
					</div>
					<div class="flex flex-wrap items-center gap-2 text-sm">
						<label id={`${rowId}-label`} for={`${rowId}-select`} class="text-surface-400">
							{m.mods_worlds_profile_label()}
						</label>
						<select
							id={`${rowId}-select`}
							aria-labelledby={gamepass
								? `${rowId}-name ${rowId}-platform ${rowId}-label`
								: `${rowId}-name ${rowId}-label`}
							class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-lg border px-3 py-1.5 text-sm focus:outline-none disabled:opacity-60"
							value={save.mod_profile?.profile_id ?? ''}
							disabled={modsState.linkingWorld[save.world_key] ?? false}
							onchange={(event) => relink(save, event.currentTarget)}
						>
							<option value="">{m.mods_worlds_no_profile()}</option>
							{#each profiles as profile (profile.id)}
								<option value={profile.id}>{profile.name}</option>
							{/each}
							{#if linkedElsewhere(save) && save.mod_profile}
								<option value={save.mod_profile.profile_id}>
									{m.mods_worlds_other_install({ name: save.mod_profile.profile_name })}
								</option>
							{/if}
						</select>
						{#if canLaunch}
							<Button
								size="sm"
								variant="primary"
								aria-label={gamepass
									? m.mods_launch_world_gamepass({ name: save.name })
									: m.mods_launch_world({ name: save.name })}
								disabled={launching}
								onclick={() => modsState.launch(targetId, save.world_key)}
							>
								<Icon icon="tabler:player-play" size={14} />
								{m.mods_launch()}
							</Button>
						{/if}
					</div>
				</div>
				{#if linkError && linkError.world_key === save.world_key}
					<p class="text-error-400 text-xs" role="alert">{worldLinkErrorText(linkError)}</p>
				{/if}
			</Card>
		</div>
	{/each}
</div>
