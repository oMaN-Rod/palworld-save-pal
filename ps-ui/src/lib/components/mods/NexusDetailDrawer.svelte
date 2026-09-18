<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { Button, Progress } from '$components/ui';
	import { downloadKey, getModsState, getNexusState } from '$states';
	import { openExternalLink } from '$lib/utils/externalLink';
	import { MessageType } from '$types';
	import type { NexusFile, NexusFileCategory } from '$types';
	import * as m from '$i18n/messages';
	import { nexusErrorText } from './nexusText';
	import { modPageUrl, nexusFileLabel } from './nexusSafety';
	import { formatDate, formatSize } from './modList';

	let { targetId, modId, onClose }: { targetId: string; modId: number; onClose: () => void } =
		$props();

	const modsState = getModsState();
	const nexusState = getNexusState();
	const progressId = $props.id();

	const CATEGORY_LABEL: Partial<Record<NexusFileCategory, () => string>> = {
		MAIN: m.mods_discover_file_main,
		UPDATE: m.mods_discover_file_update,
		OPTIONAL: m.mods_discover_file_optional,
		MISCELLANEOUS: m.mods_discover_file_misc,
		OLD_VERSION: m.mods_discover_file_old
	};

	function categoryRank(category: NexusFileCategory): number {
		return category === 'MAIN' || category === 'UPDATE' ? 0 : 1;
	}

	const name = $derived(nexusState.results.find((entry) => entry.mod_id === modId)?.name ?? '');
	const filesReply = $derived(nexusState.files[modId]);
	const filesError = $derived(modsState.lastErrorFor(MessageType.NEXUS_MOD_FILES));
	const files = $derived(
		(filesReply?.files ?? [])
			.filter((file) => Object.hasOwn(CATEGORY_LABEL, file.category))
			.slice()
			.sort((a, b) => categoryRank(a.category) - categoryRank(b.category))
	);

	/** No key sits with a free account: both take the browser path with the same premium note. */
	const isPremium = $derived(nexusState.hasKey && nexusState.account?.is_premium !== false);
	const progress = $derived(modsState.progressFor(targetId));
	const downloadError = $derived(modsState.lastErrorFor(MessageType.NEXUS_DOWNLOAD, targetId));
	/** Serializes downloads per the drawer's own mod: the server keeps no per-file lock, and
	 *  `lastRequestedFileId` below can only point at one file at a time if only one is ever in flight. */
	const anyDownloading = $derived(
		files.some((entry) => nexusState.downloadingFile(modId, entry.file_id))
	);

	/** Ties a target-scoped refusal back to the one file this drawer instance asked to download. */
	let lastRequestedFileId = $state<number | null>(null);

	$effect(() => {
		modId;
		modsState.resets;
		untrack(() => {
			lastRequestedFileId = null;
			if (nexusState.files[modId] === undefined) nexusState.loadFiles(modId);
		});
	});

	function download(file: NexusFile): void {
		if (anyDownloading) return;
		lastRequestedFileId = file.file_id;
		modsState.clearLastError(MessageType.NEXUS_DOWNLOAD, targetId);
		nexusState.startDownload({ target_id: targetId, mod_id: modId, file_id: file.file_id });
	}
</script>

<aside
	aria-label={name}
	class="bg-surface-900 border-surface-800 flex flex-col overflow-hidden rounded-lg border lg:sticky lg:top-0"
>
	<div class="flex items-start justify-between gap-2 p-4 pb-0">
		<h3 class="text-base font-bold break-words">{name}</h3>
		<button
			type="button"
			class="text-surface-400 hover:text-surface-100 shrink-0"
			aria-label={m.mods_discover_close_details({ name })}
			onclick={onClose}
		>
			<Icon icon="tabler:x" size={14} />
		</button>
	</div>

	<div class="flex flex-col gap-3 p-4 text-sm">
		<a
			href={modPageUrl(modId)}
			target="_blank"
			rel="noreferrer"
			aria-label={m.mods_discover_open_page({ name })}
			class="text-primary-400 flex items-center gap-1 text-xs hover:underline"
			onclick={(event) => openExternalLink(event, modPageUrl(modId))}
		>
			<Icon icon="tabler:external-link" size={12} />
			{m.mods_discover_open_page({ name })}
		</a>

		{#if !isPremium}
			<p class="text-surface-300 text-xs">{m.mods_discover_premium_note()}</p>
		{/if}

		{#if progress}
			<div class="flex flex-col gap-1" role="group" aria-labelledby={progressId}>
				<span id={progressId} class="text-surface-300 text-xs" aria-live="polite">
					{progress.message}
				</span>
				<Progress value={progress.pct} max={100} showLabel={false} rounded="rounded-full" />
			</div>
		{/if}

		<h4 class="text-surface-400 text-xs font-medium tracking-wide uppercase">
			{m.mods_discover_files_title()}
		</h4>

		{#if filesError && !filesReply}
			<p class="text-error-400 text-sm" role="alert">{nexusErrorText(filesError)}</p>
		{:else if files.length === 0}
			<p class="text-surface-400 py-2 text-center text-xs">{m.mods_discover_no_files()}</p>
		{:else}
			<ul class="border-surface-800 divide-surface-800 flex flex-col divide-y rounded-sm border">
				{#each files as file (file.file_id)}
					{@const label = nexusFileLabel(file)}
					{@const downloading = nexusState.downloadingFile(modId, file.file_id)}
					{@const installed = nexusState.lastInstalled[downloadKey(modId, file.file_id)]}
					{@const mine = lastRequestedFileId === file.file_id}
					<li class="flex flex-col gap-1.5 px-3 py-2 text-xs">
						<div class="flex items-center justify-between gap-2">
							<span class="font-medium">{label}</span>
							{#if CATEGORY_LABEL[file.category]}
								<span class="text-surface-500">{CATEGORY_LABEL[file.category]?.()}</span>
							{/if}
						</div>
						<div class="text-surface-500 flex items-center gap-2">
							<span>{formatSize(file.size_in_bytes)}</span>
							<span>{formatDate(new Date(file.date * 1000).toISOString())}</span>
						</div>

						{#if installed}
							<span class="text-success-400">
								{m.mods_discover_installed_version({ version: installed.version ?? '' })}
							</span>
						{/if}

						<div class="flex items-center gap-2">
							{#if isPremium}
								<Button
									size="sm"
									variant="secondary"
									disabled={anyDownloading}
									onclick={() => download(file)}
								>
									{m.mods_discover_download({ file: label })}
								</Button>
							{:else}
								<Button
									size="sm"
									variant="secondary"
									href={modPageUrl(modId, file.file_id)}
									target="_blank"
									rel="noreferrer"
									onclick={(event: MouseEvent) =>
										openExternalLink(event, modPageUrl(modId, file.file_id))}
								>
									{m.mods_discover_download_browser({ file: label })}
								</Button>
							{/if}
							{#if downloading}
								<span class="text-surface-400">{m.mods_discover_downloading({ file: label })}</span>
							{/if}
						</div>

						{#if mine && downloadError}
							{#if downloadError.code === 'premium_required'}
								<p class="text-surface-300">{m.mods_discover_premium_note()}</p>
							{:else}
								<p class="text-error-400" role="alert">{nexusErrorText(downloadError)}</p>
							{/if}
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</aside>
