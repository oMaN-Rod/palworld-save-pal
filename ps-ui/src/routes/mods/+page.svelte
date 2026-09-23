<script lang="ts">
	import type { Component } from 'svelte';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { Card } from '$components/ui';
	import { TargetList, TargetPanel } from '$components/mods';
	import NxmLinkModal from '$lib/components/mods/NxmLinkModal.svelte';
	import { getModalState, getModsState, getNexusState, getServerState } from '$states';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { isWebBuild } from '$lib/utils/platform';
	import * as m from '$i18n/messages';

	const modsState = getModsState();
	const nexusState = getNexusState();
	const serverState = getServerState();
	const remoteMode = getRemoteMode();
	const modal = getModalState();

	const available = $derived(!isWebBuild || remoteMode.active);
	const requestedId = $derived(page.url.searchParams.get('target'));
	const selected = $derived(
		modsState.targets.find((target) => target.id === requestedId) ?? modsState.targets[0]
	);

	$effect(() => {
		if (!available) return;
		const resets = modsState.resets;
		untrack(() => {
			modsState.loadTargets();
			modsState.loadLibrary();
			serverState.loadServers();
			modsState.subscribeVerification();
			if (nexusState.shouldForget(resets)) nexusState.forgetInFlight();
			if (PUBLIC_DESKTOP_MODE === 'true' && !remoteMode.active) nexusState.subscribeLinks();
		});
	});

	/** One at a time, oldest first: dismissing only after the modal closes keeps the queue from skipping. */
	let linkModalOpen = false;
	$effect(() => {
		const link = nexusState.links[0];
		if (!link || linkModalOpen) return;
		untrack(() => {
			linkModalOpen = true;
			modal
				.showModal(NxmLinkModal as unknown as Component, {
					payload: link,
					defaultTargetId: selected?.id
				})
				.then(() => {
					linkModalOpen = false;
					nexusState.dismissLink(0);
				});
		});
	});

	/** Only a change in _which_ session is talking clears Nexus state; a reconnect over the same one must not. */
	let lastRemoteActive: boolean | undefined;
	$effect(() => {
		const active = remoteMode.active;
		untrack(() => {
			const changed = lastRemoteActive !== undefined && active !== lastRemoteActive;
			lastRemoteActive = active;
			if (changed) nexusState.reset();
		});
	});

	$effect(() => {
		if (!modsState.targetsLoaded || requestedId === null) return;
		if (modsState.targets.some((target) => target.id === requestedId)) return;
		untrack(() => select(selected?.id ?? null));
	});

	function select(targetId: string | null) {
		const url = new URL(page.url);
		if (targetId) url.searchParams.set('target', targetId);
		else url.searchParams.delete('target');
		goto(url, { replaceState: true, keepFocus: true, noScroll: true });
	}
</script>

{#if available}
	<div
		id="mods-shell"
		class="flex h-full min-h-[calc(100vh-var(--titlebar-h))] w-full flex-col gap-4 p-4 md:flex-row"
	>
		<div id="mods-target-list" class="flex w-full shrink-0 flex-col gap-4 md:w-80">
			<TargetList selectedId={selected?.id} onselect={select} />
		</div>

		<div class="min-w-0 flex-1">
			{#if selected}
				<TargetPanel targetId={selected.id} />
			{:else}
				<div class="text-surface-400 flex h-full items-center justify-center">
					<div class="text-center">
						<Icon icon="tabler:package" size={48} class="mx-auto mb-4 opacity-30" />
						<p class="text-lg">{m.mods_select_target()}</p>
					</div>
				</div>
			{/if}
		</div>
	</div>
{:else}
	<div class="flex h-full w-full items-center justify-center p-4">
		<Card class="text-surface-400 max-w-md text-center">
			<Icon icon="tabler:package" size={32} class="mx-auto mb-2 opacity-50" />
			<p>{m.mods_unavailable_title()}</p>
			<p class="mt-1 text-sm">{m.mods_unavailable_hint()}</p>
		</Card>
	</div>
{/if}
