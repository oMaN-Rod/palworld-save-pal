<script lang="ts">
	import { onMount } from 'svelte';
	import { Button, Card, Input } from '$components/ui';
	import { focusModal } from '$utils/modalUtils';
	import { getModsState, getNexusState } from '$states';
	import { openExternalLink } from '$lib/utils/externalLink';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { nexusErrorText } from './nexusText';

	let { closeModal }: { closeModal: () => void } = $props();

	const KEY_PAGE_URL = 'https://www.nexusmods.com/users/myaccount?tab=api';

	const modsState = getModsState();
	const nexusState = getNexusState();

	/** Local only: passed straight to setKey and cleared in the same call, never stored elsewhere. */
	let key = $state('');
	let modalContainer: HTMLDivElement;

	const busy = $derived(nexusState.isBusy('account'));
	const refusal = $derived(modsState.lastErrorFor(MessageType.NEXUS_KEY_SET));

	let awaitingSave = false;

	function save(): void {
		if (key.trim().length === 0 || busy) return;
		awaitingSave = true;
		nexusState.setKey(key);
		key = '';
	}

	/** Closes the modal once the save it started settles cleanly; a refusal keeps it open to show it. */
	$effect(() => {
		if (busy || !awaitingSave) return;
		const failed = refusal !== undefined;
		awaitingSave = false;
		if (!failed) closeModal();
	});

	function isOutsideContent(target: EventTarget | null): boolean {
		const dialog = modalContainer.closest('[role="dialog"]');
		return (
			target instanceof Node &&
			dialog !== null &&
			dialog.contains(target) &&
			!modalContainer.contains(target)
		);
	}

	/** A reply to an abandoned save would otherwise be taken for a reopened modal's own. */
	function blockDismissal(event: Event) {
		if (!busy) return;
		const dismissing =
			event instanceof KeyboardEvent ? event.key === 'Escape' : isOutsideContent(event.target);
		if (!dismissing) return;
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	onMount(() => {
		modsState.clearLastError(MessageType.NEXUS_KEY_SET);
		focusModal(modalContainer);
		window.addEventListener('keydown', blockDismissal, true);
		window.addEventListener('click', blockDismissal, true);
		return () => {
			window.removeEventListener('keydown', blockDismissal, true);
			window.removeEventListener('click', blockDismissal, true);
		};
	});
</script>

<div bind:this={modalContainer}>
	<Card class="flex w-[440px] max-w-full flex-col gap-4">
		<h3 class="h3">{m.mods_nexus_key_title()}</h3>
		<p class="text-surface-300 text-sm">{m.mods_nexus_key_body()}</p>
		<Input
			type="password"
			label={m.mods_nexus_key_label()}
			bind:value={key}
			autocomplete="off"
			spellcheck="false"
			disabled={busy}
		/>
		<a
			href={KEY_PAGE_URL}
			target="_blank"
			rel="noreferrer"
			class="text-primary-400 self-start text-xs hover:underline"
			onclick={(event) => openExternalLink(event, KEY_PAGE_URL)}
		>
			{m.mods_nexus_key_where()}
		</a>
		{#if refusal}
			<p class="text-error-400 text-sm" role="alert">{nexusErrorText(refusal)}</p>
		{/if}
		<div class="flex justify-end gap-2">
			<Button variant="ghost" disabled={busy} onclick={() => closeModal()}>
				{m.mods_nexus_key_cancel()}
			</Button>
			<Button
				variant="primary"
				disabled={busy || key.trim().length === 0}
				loading={busy}
				onclick={save}
				data-modal-primary
			>
				{m.mods_nexus_key_save()}
			</Button>
		</div>
	</Card>
</div>
