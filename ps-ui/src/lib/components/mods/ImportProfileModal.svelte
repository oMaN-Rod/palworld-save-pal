<script lang="ts">
	import { onMount } from 'svelte';
	import { Button, Card, Input } from '$components/ui';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import ImportResult from './ImportResult.svelte';
	import UploadPicker from './UploadPicker.svelte';
	import { importErrorText } from './shareText';

	let { targetId, closeModal }: { targetId: string; closeModal: () => void } = $props();

	const SELECT_FILE = '__select__';
	const modsState = getModsState();
	const remoteMode = getRemoteMode();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let path = $state('');
	let name = $state('');
	let sent = $state(false);
	let modalContainer: HTMLDivElement;

	const importing = $derived(modsState.importing[targetId] ?? false);
	const result = $derived(sent ? modsState.lastImport[targetId] : undefined);
	const refusal = $derived(
		sent ? modsState.lastErrorFor(MessageType.PROFILE_IMPORT, targetId) : undefined
	);

	function importFrom(source: string) {
		if (!source || importing) return;
		sent = true;
		modsState.importProfile(targetId, source, name);
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

	/** Closing before the import replies would leave its outcome with nowhere to show. */
	function blockDismissal(event: Event) {
		if (!importing) return;
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
	<Card class="flex max-h-[85vh] w-[560px] max-w-full flex-col gap-4 overflow-y-auto">
		<h3 class="h3">{m.mods_import_title()}</h3>
		{#if result}
			<ImportResult {result} />
			<div class="flex justify-end">
				<Button variant="ghost" onclick={() => closeModal()} data-modal-primary
					>{m.mods_close()}</Button
				>
			</div>
		{:else}
			<Input label={m.mods_import_name_label()} bind:value={name} disabled={importing} />
			{#if remoteMode.active}
				<UploadPicker
					{targetId}
					purpose="import"
					accept=".psmods"
					label={m.mods_upload_import_label()}
					onUploaded={importFrom}
				/>
			{:else if isDesktopMode}
				<Button
					variant="primary"
					class="self-start"
					disabled={importing}
					onclick={() => importFrom(SELECT_FILE)}
					data-modal-primary
				>
					{m.mods_import_choose()}
				</Button>
			{:else}
				<div class="flex items-end gap-2">
					<div class="grow">
						<Input
							label={m.mods_import_path_label()}
							placeholder={m.mods_import_path_placeholder()}
							bind:value={path}
							disabled={importing}
						/>
					</div>
					<Button
						variant="primary"
						class="mb-2"
						disabled={importing || path.trim().length === 0}
						onclick={() => importFrom(path.trim())}
						data-modal-primary
					>
						{m.mods_import_submit()}
					</Button>
				</div>
			{/if}
			{#if importing}
				<p class="text-surface-400 text-sm" role="status">{m.mods_import_busy()}</p>
			{/if}
			{#if refusal}
				<p class="text-error-400 text-sm" role="alert">{importErrorText(refusal)}</p>
			{/if}
			<div class="flex justify-end">
				<Button variant="ghost" disabled={importing} onclick={() => closeModal()}>
					{m.mods_close()}
				</Button>
			</div>
		{/if}
	</Card>
</div>
