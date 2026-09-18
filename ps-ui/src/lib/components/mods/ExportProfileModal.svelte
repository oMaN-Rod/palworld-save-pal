<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Input } from '$components/ui';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { versionName } from './applyOutcome';
	import { displayName } from './modList';
	import { exportErrorText } from './shareText';

	let {
		targetId,
		profileId,
		profileName,
		closeModal
	}: { targetId: string; profileId: string; profileName: string; closeModal: () => void } =
		$props();

	const SELECT_FILE = '__select__';
	const modsState = getModsState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let includeArchives = $state(true);
	let path = $state('');
	let sent = $state(false);
	let modalContainer: HTMLDivElement;

	const exporting = $derived(modsState.exporting[targetId] ?? false);
	const result = $derived(sent ? modsState.lastExport[targetId] : undefined);
	const refusal = $derived(
		sent ? modsState.lastErrorFor(MessageType.PROFILE_EXPORT, targetId) : undefined
	);

	function modName(modId: string): string {
		const mod = modsState.mods.find((entry) => entry.id === modId);
		return mod ? displayName(mod) : modId;
	}

	function exportTo(destination: string) {
		if (!destination || exporting) return;
		sent = true;
		modsState.exportProfile(targetId, profileId, includeArchives, destination);
	}

	function isOutsideContent(eventTarget: EventTarget | null): boolean {
		const dialog = modalContainer.closest('[role="dialog"]');
		return (
			eventTarget instanceof Node &&
			dialog !== null &&
			dialog.contains(eventTarget) &&
			!modalContainer.contains(eventTarget)
		);
	}

	/** Closing before the export replies would leave its outcome with nowhere to show. */
	function blockDismissal(event: Event) {
		if (!exporting) return;
		const dismissing =
			event instanceof KeyboardEvent ? event.key === 'Escape' : isOutsideContent(event.target);
		if (!dismissing) return;
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	onMount(() => {
		window.addEventListener('keydown', blockDismissal, true);
		window.addEventListener('click', blockDismissal, true);
		return () => {
			window.removeEventListener('keydown', blockDismissal, true);
			window.removeEventListener('click', blockDismissal, true);
		};
	});
</script>

<div bind:this={modalContainer}>
	<Card class="flex max-h-[85vh] w-[520px] max-w-full flex-col gap-4 overflow-y-auto">
		<h3 class="h3">{m.mods_export_title({ name: profileName })}</h3>
		{#if result}
			<div class="flex flex-col gap-2 text-sm" role="status">
				<p class="text-success-400 flex items-center gap-2">
					<Icon icon="tabler:check" size={14} class="shrink-0" />
					<span>{m.mods_export_done({ count: result.entries, path: result.path })}</span>
				</p>
				<p class="text-surface-300">
					{m.mods_export_archives_included({ count: result.archives_included })}
				</p>
				{#if result.missing_archives.length > 0}
					<p class="text-warning-400">{m.mods_export_missing_archives()}</p>
					<ul class="list-disc pl-5">
						{#each result.missing_archives as versionId (versionId)}
							<li>{versionName(modsState.mods, versionId)}</li>
						{/each}
					</ul>
				{/if}
				{#if result.unresolved.length > 0}
					<p class="text-warning-400">{m.mods_export_unresolved()}</p>
					<ul class="list-disc pl-5">
						{#each result.unresolved as modId (modId)}
							<li>{modName(modId)}</li>
						{/each}
					</ul>
				{/if}
			</div>
			<div class="flex justify-end">
				<Button variant="ghost" onclick={() => closeModal()} data-modal-primary
					>{m.mods_close()}</Button
				>
			</div>
		{:else}
			<label class="flex w-fit cursor-pointer items-center gap-2 text-sm">
				<input
					type="checkbox"
					class="accent-primary-500 size-4"
					bind:checked={includeArchives}
					disabled={exporting}
				/>
				{m.mods_export_include_archives()}
			</label>
			<p class="text-surface-400 text-xs">{m.mods_export_include_archives_hint()}</p>
			{#if isDesktopMode}
				<Button
					variant="primary"
					class="flex items-center gap-2 self-start"
					disabled={exporting}
					onclick={() => exportTo(SELECT_FILE)}
					data-modal-primary
				>
					<Icon icon="tabler:file-export" size={14} />
					{m.mods_export_choose()}
				</Button>
			{:else}
				<div class="flex items-end gap-2">
					<div class="grow">
						<Input
							label={m.mods_export_path_label()}
							placeholder={m.mods_export_path_placeholder()}
							bind:value={path}
							disabled={exporting}
						/>
					</div>
					<Button
						variant="primary"
						class="mb-2"
						disabled={exporting || path.trim().length === 0}
						onclick={() => exportTo(path.trim())}
						data-modal-primary
					>
						{m.mods_export_submit()}
					</Button>
				</div>
			{/if}
			{#if refusal}
				<p class="text-error-400 text-sm" role="alert">{exportErrorText(refusal)}</p>
			{/if}
			<div class="flex justify-end">
				<Button variant="ghost" disabled={exporting} onclick={() => closeModal()}>
					{m.mods_close()}
				</Button>
			</div>
		{/if}
	</Card>
</div>
