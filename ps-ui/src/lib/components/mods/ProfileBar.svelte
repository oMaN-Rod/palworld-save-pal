<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Component } from 'svelte';
	import { Button } from '$components/ui';
	import { cn } from '$theme';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { getModalState, getModsState } from '$states';
	import { MessageType } from '$types';
	import type { RecordedModError } from '$types';
	import * as m from '$i18n/messages';
	import ActionMenu, { menuItemClass } from './ActionMenu.svelte';
	import ExportProfileModal from './ExportProfileModal.svelte';
	import ImportProfileModal from './ImportProfileModal.svelte';
	import ProfileNameModal, { type ProfileNameResult } from './ProfileNameModal.svelte';
	import { profileErrorText } from './profileText';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const modal = getModalState();
	const remoteMode = getRemoteMode();
	const selectId = $props.id();

	const profileTypes = [
		MessageType.PROFILE_CREATE,
		MessageType.PROFILE_RENAME,
		MessageType.PROFILE_DELETE,
		MessageType.PROFILE_ACTIVATE
	];

	const profiles = $derived(modsState.profiles[targetId] ?? []);
	const viewed = $derived(modsState.viewedProfile(targetId));
	const managing = $derived(modsState.managingProfile[targetId] ?? false);
	const activating = $derived(modsState.activating[targetId] ?? false);
	const worlds = $derived(viewed?.worlds ?? []);
	const refusals = $derived(
		profileTypes
			.map((type) => ({ type, error: modsState.lastErrorFor(type, targetId) }))
			.filter(
				(entry): entry is { type: MessageType; error: RecordedModError } =>
					entry.error !== undefined
			)
	);

	function clearRefusals() {
		for (const type of profileTypes) modsState.clearLastError(type, targetId);
	}

	function askName(props: Record<string, unknown>): Promise<ProfileNameResult | null> {
		return modal.showModal<ProfileNameResult | null>(
			ProfileNameModal as unknown as Component,
			props
		);
	}

	async function create() {
		const source = viewed;
		const result = await askName({
			title: m.mods_profile_new_title(),
			confirmText: m.mods_profile_create(),
			copyFrom: source?.name
		});
		if (!result) return;
		clearRefusals();
		modsState.createProfile(targetId, result.name, result.copy ? source?.id : undefined);
	}

	async function rename() {
		const profile = viewed;
		if (!profile) return;
		const result = await askName({
			title: m.mods_profile_rename_title(),
			confirmText: m.mods_profile_rename(),
			initialName: profile.name
		});
		if (!result) return;
		clearRefusals();
		modsState.renameProfile(targetId, profile.id, result.name);
	}

	async function remove() {
		const profile = viewed;
		if (!profile || profile.is_default) return;
		const confirmed = await modal.showConfirmModal({
			title: m.mods_profile_delete_title(),
			message: profile.is_active
				? m.mods_profile_delete_active_message({ name: profile.name })
				: m.mods_profile_delete_message({ name: profile.name }),
			confirmText: m.mods_profile_delete(),
			cancelText: m.mods_panel_cancel()
		});
		if (!confirmed) return;
		clearRefusals();
		modsState.deleteProfile(targetId, profile.id);
	}

	function activate() {
		if (!viewed) return;
		clearRefusals();
		modsState.activateProfile(targetId, viewed.id);
	}

	function openExport() {
		if (!viewed) return;
		modal.showModal(ExportProfileModal as unknown as Component, {
			targetId,
			profileId: viewed.id,
			profileName: viewed.name
		});
	}

	function openImport() {
		modal.showModal(ImportProfileModal as unknown as Component, { targetId });
	}
</script>

{#if viewed}
	<div class="flex min-w-0 flex-col gap-1">
		<div class="flex flex-wrap items-center gap-2">
			<label for={selectId} class="sr-only">{m.mods_profile_label()}</label>
			<select
				id={selectId}
				class="bg-surface-900 border-surface-700 text-surface-100 hover:border-surface-500 focus:border-primary-500 rounded-full border py-1 pr-8 pl-3 text-sm font-medium focus:outline-none"
				value={viewed.id}
				onchange={(event) => modsState.viewProfile(targetId, event.currentTarget.value)}
			>
				{#each profiles as profile (profile.id)}
					<option value={profile.id}>
						{profile.is_active
							? m.mods_profile_option_active({ name: profile.name })
							: profile.name}
					</option>
				{/each}
			</select>
			{#if viewed.is_active}
				<span class="bg-success-500/20 text-success-400 rounded-xs px-2 py-0.5 text-xs font-medium">
					{m.mods_profile_active()}
				</span>
			{:else}
				<Button
					size="sm"
					variant="outline"
					disabled={activating}
					loading={activating}
					aria-label={m.mods_profile_activate_label()}
					onclick={activate}
				>
					{m.mods_profile_activate()}
				</Button>
			{/if}
			<ActionMenu label={m.mods_profile_actions()} position="bottom-start">
				{#snippet items({ close })}
					<button
						role="menuitem"
						class={menuItemClass}
						disabled={managing}
						onclick={() => {
							close();
							create();
						}}
					>
						<Icon icon="tabler:plus" size={14} />
						{m.mods_profile_new_title()}
					</button>
					<button
						role="menuitem"
						class={menuItemClass}
						disabled={managing}
						onclick={() => {
							close();
							rename();
						}}
					>
						<Icon icon="tabler:pencil" size={14} />
						{m.mods_profile_rename_title()}
					</button>
					{#if !remoteMode.active}
						<button
							role="menuitem"
							class={menuItemClass}
							onclick={() => {
								close();
								openExport();
							}}
						>
							<Icon icon="tabler:file-export" size={14} />
							{m.mods_profile_export_label()}
						</button>
					{/if}
					<button
						role="menuitem"
						class={menuItemClass}
						onclick={() => {
							close();
							openImport();
						}}
					>
						<Icon icon="tabler:file-import" size={14} />
						{m.mods_profile_import_label()}
					</button>
					<div class="border-surface-700 my-1 border-t" role="separator"></div>
					<button
						role="menuitem"
						class={cn(menuItemClass, 'text-red-400')}
						disabled={managing || viewed.is_default}
						aria-describedby={viewed.is_default ? `${selectId}-default` : undefined}
						onclick={() => {
							close();
							remove();
						}}
					>
						<Icon icon="tabler:trash-x" size={14} />
						{m.mods_profile_delete_title()}
					</button>
					{#if viewed.is_default}
						<p id={`${selectId}-default`} class="sr-only">{m.mods_profile_default_undeletable()}</p>
					{/if}
				{/snippet}
			</ActionMenu>
		</div>
		{#if worlds.length > 0}
			<p class="text-surface-400 text-xs">
				{m.mods_profile_worlds({ worlds: worlds.map((world) => world.world_name).join(', ') })}
			</p>
		{/if}
		{#each refusals as { type, error } (type)}
			<p class="text-error-400 flex items-center gap-2 text-sm" role="alert">
				<Icon icon="tabler:alert-circle" size={14} class="shrink-0" />
				{profileErrorText(error)}
			</p>
		{/each}
	</div>
{/if}
