<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { getModalState, getModsState } from '$states';
	import type { LibraryMod, ModVersion } from '$types';
	import * as m from '$i18n/messages';
	import ModTile, { modTypeText } from './ModTile.svelte';
	import { modTypeLabel as typeLabel } from './modLabels';
	import {
		currentVersion,
		displayName,
		formatDate,
		formatSize,
		isSubscribed,
		totalSize,
		versionsNewestFirst
	} from './modList';

	let { mod, targetId, onClose }: { mod: LibraryMod; targetId: string; onClose: () => void } =
		$props();

	const modsState = getModsState();
	const modal = getModalState();
	const pinId = $props.id();

	const name = $derived(displayName(mod));
	const profile = $derived(modsState.viewedProfile(targetId));
	const busy = $derived(modsState.settingMod[targetId] ?? false);
	const subscribed = $derived(isSubscribed(mod));
	const version = $derived(currentVersion(mod));
	const versions = $derived(versionsNewestFirst(mod));
	const pinnedId = $derived(profile?.mods.find((entry) => entry.mod_id === mod.id)?.mod_version_id);

	async function deleteVersion(entry: ModVersion) {
		const confirmed = await modal.showConfirmModal({
			title: m.mods_list_delete_version_title(),
			message: m.mods_list_delete_version_message({ version: entry.version, name }),
			confirmText: m.mods_list_delete_version(),
			cancelText: m.mods_panel_cancel()
		});
		if (confirmed) modsState.deleteVersion(entry.id);
	}
</script>

<aside
	aria-label={name}
	class="bg-surface-900 border-surface-800 flex flex-col overflow-hidden rounded-lg border lg:sticky lg:top-0"
>
	<ModTile {mod} class="aspect-2/1 w-full text-5xl">
		<span
			class={[
				'bg-surface-950/80 absolute top-2 left-2 rounded-xs px-1.5 py-0.5 text-[10px] font-bold tracking-wide uppercase backdrop-blur-sm',
				modTypeText[mod.mod_type] ?? 'text-surface-300'
			]}
		>
			{typeLabel(mod.mod_type)}
		</span>
		<button
			type="button"
			class="bg-surface-950/70 absolute top-2 right-2 backdrop-blur-sm"
			aria-label={m.mods_details_close()}
			onclick={onClose}
		>
			<Icon icon="tabler:x" size={14} />
		</button>
	</ModTile>

	<div class="flex flex-col gap-4 p-4 text-sm">
		<div class="flex flex-col gap-1">
			<h3 class="text-base font-bold break-words">{name}</h3>
			<p class="text-surface-400 flex flex-wrap gap-x-2 text-xs">
				{#if mod.author}
					<span>{m.mods_panel_by_author({ author: mod.author })}</span>
				{/if}
				{#if version}
					<span>{m.mods_review_version({ version: version.version })}</span>
				{/if}
				<span>{formatSize(totalSize(mod))}</span>
			</p>
			{#if mod.summary}
				<p class="text-surface-300 mt-1 text-xs">{mod.summary}</p>
			{/if}
		</div>

		{#if profile && !subscribed && mod.versions.length > 0}
			<div class="flex flex-col gap-1.5">
				<label for={pinId} class="text-surface-400 text-xs font-medium tracking-wide uppercase">
					{m.mods_pin_label()}
				</label>
				<select
					id={pinId}
					class="bg-surface-800 border-surface-600 text-surface-100 focus:border-primary-500 rounded-sm border px-2 py-1.5 text-sm focus:outline-none disabled:opacity-60"
					value={pinnedId ?? ''}
					disabled={busy}
					onchange={(event) =>
						modsState.setModVersion(
							targetId,
							profile.id,
							mod.id,
							modsState.enabledIn(targetId, profile.id, mod.id),
							event.currentTarget.value || null
						)}
				>
					<option value="">
						{version
							? m.mods_pin_follow_current_version({ version: version.version })
							: m.mods_pin_follow_current()}
					</option>
					{#each versions as entry (entry.id)}
						<option value={entry.id}>{m.mods_pin_version({ version: entry.version })}</option>
					{/each}
				</select>
			</div>
		{/if}

		{#if !subscribed && versions.length > 0}
			<div class="flex flex-col gap-1.5">
				<h4 class="text-surface-400 text-xs font-medium tracking-wide uppercase">
					{m.mods_details_versions()}
				</h4>
				<ul class="border-surface-800 divide-surface-800 flex flex-col divide-y rounded-sm border">
					{#each versions as entry (entry.id)}
						{@const isCurrent = entry.id === mod.current_version_id}
						<li data-version-id={entry.id} class="flex flex-col gap-1 px-3 py-2 text-xs">
							<div class="flex items-center gap-2">
								<span class="font-mono">{entry.version}</span>
								{#if isCurrent}
									<span class="bg-primary-500/20 text-primary-400 rounded-xs px-1">
										{m.mods_list_current()}
									</span>
								{/if}
								{#if entry.id === pinnedId}
									<Icon icon="tabler:pin" size={12} class="text-primary-300" />
								{/if}
								<span class="text-surface-500 ml-auto">{formatSize(entry.size_bytes)}</span>
							</div>
							<div class="flex items-center gap-1">
								<span class="text-surface-500 flex-1">
									{m.mods_list_installed({ date: formatDate(entry.installed_at) })}
								</span>
								{#if !isCurrent}
									<Button
										variant="ghost"
										size="sm"
										onclick={() => modsState.setCurrentVersion(mod.id, entry.id)}
									>
										{m.mods_list_make_current()}
									</Button>
									<Button variant="ghost" size="sm" onclick={() => deleteVersion(entry)}>
										<Icon icon="tabler:trash-x" size={12} class="text-red-400" />
										{m.mods_list_delete_version()}
									</Button>
								{/if}
							</div>
						</li>
					{/each}
				</ul>
			</div>
		{/if}
	</div>
</aside>
