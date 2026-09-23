<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Input, Spinner } from '$components/ui';
	import { onMount, untrack } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { hazardText, normaliseRoot, platformLabel } from './targets';

	let { closeModal }: { closeModal: (targetId: string | null) => void } = $props();

	const SELECT_FOLDER = '__select__';

	const modsState = getModsState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let modalContainer: HTMLDivElement;
	let path = $state('');
	let sentRoot = $state<string | null>(null);
	let sentSeq = $state<number | null>(null);

	const busy = $derived(sentSeq !== null);
	const registered = $derived(
		new Set(modsState.targets.map((target) => normaliseRoot(target.root_path)))
	);
	const refusal = $derived(modsState.lastErrorFor(MessageType.MOD_TARGET_ADD));

	function add(root: string) {
		if (!root || busy) return;
		modsState.clearLastError(MessageType.MOD_TARGET_ADD);
		sentSeq = modsState.addReply.seq;
		sentRoot = root;
		modsState.addTarget(root);
	}

	$effect(() => {
		const reply = modsState.addReply;
		if (sentSeq === null || reply.seq === sentSeq) return;
		untrack(() => {
			sentSeq = null;
			sentRoot = null;
			if (reply.targetId) closeModal(reply.targetId);
		});
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

	/** A reply to an abandoned add would otherwise be taken for a reopened modal's own. */
	function blockDismissal(event: Event) {
		if (!busy) return;
		const dismissing =
			event instanceof KeyboardEvent ? event.key === 'Escape' : isOutsideContent(event.target);
		if (!dismissing) return;
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	onMount(() => {
		modsState.clearLastError(MessageType.MOD_TARGET_ADD);
		modsState.detect();
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
	<Card class="modal-panel flex max-w-[640px] min-w-[520px] flex-col gap-4">
		<h3 class="h3">{m.mods_add_target_title()}</h3>

		<section class="flex flex-col gap-2">
			<h4 class="text-surface-400 text-xs font-medium uppercase">{m.mods_detected_title()}</h4>
			{#if modsState.detecting}
				<p class="text-surface-400 flex items-center gap-2 text-sm">
					<Spinner size="size-4" />
					<span>{m.mods_detecting()}</span>
				</p>
			{:else if modsState.detected.length === 0}
				<p class="text-surface-400 text-sm">{m.mods_detected_empty()}</p>
			{:else}
				{#each modsState.detected as install (install.root)}
					<div class="bg-surface-800 flex flex-col gap-1 rounded-sm p-3">
						<div class="flex items-center gap-2">
							<span class="min-w-0 flex-1 truncate font-mono text-sm" title={install.root}>
								{install.root}
							</span>
							<span class="text-surface-400 shrink-0 text-xs">
								{platformLabel(install.platform)}
							</span>
							{#if registered.has(normaliseRoot(install.root))}
								<span class="flex shrink-0 items-center gap-1 text-sm text-green-400">
									<Icon icon="tabler:check" size={14} />
									{m.mods_target_added()}
								</span>
							{:else}
								<Button
									variant="secondary"
									size="sm"
									disabled={busy}
									loading={sentRoot === install.root}
									onclick={() => add(install.root)}
								>
									{m.mods_target_add()}
								</Button>
							{/if}
						</div>
						{#each install.hazards as hazard (hazard)}
							<p class="text-warning-400 flex items-start gap-2 text-xs">
								<Icon icon="tabler:alert-triangle" size={14} class="mt-0.5 shrink-0" />
								{hazardText(hazard)}
							</p>
						{/each}
					</div>
				{/each}
			{/if}
		</section>

		{#if isDesktopMode}
			<Button
				variant="neutral"
				class="flex items-center gap-2 self-start"
				disabled={busy}
				loading={sentRoot === SELECT_FOLDER}
				onclick={() => add(SELECT_FOLDER)}
			>
				<Icon icon="tabler:folder-open" size={14} />
				{m.mods_choose_folder()}
			</Button>
		{:else}
			<div class="flex items-end gap-2">
				<div class="grow">
					<Input
						label={m.mods_target_path_label()}
						bind:value={path}
						placeholder={m.mods_target_path_placeholder()}
					/>
				</div>
				<Button
					variant="secondary"
					class="mb-2"
					disabled={busy || path.trim().length === 0}
					loading={sentRoot !== null && sentRoot === path.trim()}
					onclick={() => add(path.trim())}
					data-modal-primary
				>
					{m.mods_target_add()}
				</Button>
			</div>
		{/if}

		{#if refusal}
			<p class="text-error-400 text-sm" role="alert">{refusal.message}</p>
		{/if}

		<div class="flex justify-end">
			<Button variant="ghost" disabled={busy} onclick={() => closeModal(null)}>
				{m.mods_close()}
			</Button>
		</div>
	</Card>
</div>
