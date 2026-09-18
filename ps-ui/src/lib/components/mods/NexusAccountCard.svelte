<script lang="ts">
	import { untrack, type Component } from 'svelte';
	import { Button, Card } from '$components/ui';
	import { getModalState, getModsState, getNexusState } from '$states';
	import { openExternalLink } from '$lib/utils/externalLink';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import NexusKeyModal from './NexusKeyModal.svelte';
	import { nexusErrorText } from './nexusText';
	import { safeHttpUrl } from './nexusSafety';

	const modsState = getModsState();
	const nexusState = getNexusState();
	const modal = getModalState();

	const account = $derived(nexusState.account);
	const rateLimit = $derived(nexusState.rateLimit);
	const handler = $derived(nexusState.handler);

	const accountRefusal = $derived(modsState.lastErrorFor(MessageType.NEXUS_ACCOUNT_GET));

	const profileUrl = $derived(safeHttpUrl(account?.profile_url ?? null));

	const tierLabel = $derived(
		account?.is_premium
			? m.mods_nexus_account_premium()
			: account?.is_supporter
				? m.mods_nexus_account_supporter()
				: m.mods_nexus_account_free()
	);

	const showRateLimit = $derived(
		rateLimit !== null &&
			(rateLimit.hourly_remaining !== null || rateLimit.daily_remaining !== null)
	);

	const handlerRefusal = $derived(modsState.lastErrorFor(MessageType.NEXUS_HANDLER_REGISTER));
	/** A race between a stale `foreign: false` status and a fresh `foreign_handler` refusal
	 *  is resolved in the refusal's favor: it is the more recent truth. */
	const refusedForeignCurrent = $derived(
		handlerRefusal?.code === 'foreign_handler' && typeof handlerRefusal.current === 'string'
			? handlerRefusal.current
			: null
	);
	const isForeign = $derived(refusedForeignCurrent !== null || (handler?.foreign ?? false));
	const foreignCurrent = $derived(refusedForeignCurrent ?? handler?.current ?? null);
	const otherHandlerRefusal = $derived(
		handlerRefusal && handlerRefusal.code !== 'foreign_handler' ? handlerRefusal : undefined
	);

	$effect(() => {
		modsState.resets;
		untrack(() => {
			nexusState.loadAccount();
			nexusState.loadHandler();
		});
	});

	function openKeyModal(): void {
		modal.showModal(NexusKeyModal as unknown as Component, {});
	}

	async function replaceHandler(): Promise<void> {
		const current = foreignCurrent ?? '';
		const confirmed = await modal.showConfirmModal({
			title: m.mods_nexus_handler_replace(),
			message: m.mods_nexus_handler_foreign({ current }),
			confirmText: m.mods_nexus_handler_replace(),
			cancelText: m.mods_panel_cancel()
		});
		if (!confirmed) return;
		nexusState.registerHandler(true);
	}

	async function confirmClearKey(): Promise<void> {
		const confirmed = await modal.showConfirmModal({
			title: m.mods_nexus_account_clear_confirm_title(),
			message: m.mods_nexus_account_clear_confirm_message(),
			confirmText: m.mods_nexus_account_clear_key(),
			cancelText: m.mods_panel_cancel()
		});
		if (!confirmed) return;
		nexusState.clearKey();
	}
</script>

<Card class="flex flex-col gap-4">
	<div class="flex flex-col gap-2">
		<h3 class="text-surface-400 text-xs font-medium tracking-wide uppercase">
			{m.mods_nexus_account_title()}
		</h3>
		{#if !nexusState.hasKey}
			<p class="text-surface-300 text-sm">{m.mods_nexus_account_anonymous()}</p>
			<Button variant="secondary" size="sm" class="self-start" onclick={openKeyModal}>
				{m.mods_nexus_account_add_key()}
			</Button>
		{:else if account}
			<div class="flex flex-wrap items-center gap-2 text-sm">
				<span class="font-medium">{account.name}</span>
				<span
					class="bg-primary-500/15 text-primary-400 rounded-xs px-1.5 py-0.5 text-[10px] font-medium uppercase"
				>
					{tierLabel}
				</span>
				{#if profileUrl}
					<a
						href={profileUrl}
						target="_blank"
						rel="noreferrer"
						class="text-primary-400 text-xs hover:underline"
						aria-label={m.mods_nexus_account_profile({ name: account.name })}
						onclick={(event) => openExternalLink(event, profileUrl)}
					>
						{m.mods_nexus_account_profile({ name: account.name })}
					</a>
				{/if}
			</div>
			<div class="flex gap-2">
				<Button variant="secondary" size="sm" onclick={openKeyModal}>
					{m.mods_nexus_account_replace_key()}
				</Button>
				<Button variant="ghost" size="sm" onclick={confirmClearKey}>
					{m.mods_nexus_account_clear_key()}
				</Button>
			</div>
		{/if}
		{#if accountRefusal}
			<p class="text-error-400 text-sm" role="alert">{nexusErrorText(accountRefusal)}</p>
		{/if}
	</div>

	{#if showRateLimit && rateLimit}
		<p class="text-surface-400 text-xs">
			{m.mods_nexus_limit({
				hourly: rateLimit.hourly_remaining !== null ? String(rateLimit.hourly_remaining) : '—',
				daily: rateLimit.daily_remaining !== null ? String(rateLimit.daily_remaining) : '—'
			})}
		</p>
	{/if}

	{#if handler}
		<div class="border-surface-800 flex flex-col gap-2 border-t pt-3">
			<h4 class="text-surface-400 text-xs font-medium tracking-wide uppercase">
				{m.mods_nexus_handler_title()}
			</h4>
			{#if !handler.supported}
				<p class="text-surface-300 text-sm">{m.mods_nexus_handler_unsupported()}</p>
			{:else if isForeign}
				<p class="text-surface-300 text-sm">
					{m.mods_nexus_handler_foreign({ current: foreignCurrent ?? '' })}
				</p>
				<Button variant="secondary" size="sm" class="self-start" onclick={replaceHandler}>
					{m.mods_nexus_handler_replace()}
				</Button>
			{:else if handler.registered}
				<p class="text-surface-300 text-sm">{m.mods_nexus_handler_registered()}</p>
			{:else}
				<p class="text-surface-300 text-sm">{m.mods_nexus_handler_unregistered()}</p>
				<Button
					variant="secondary"
					size="sm"
					class="self-start"
					onclick={() => nexusState.registerHandler(false)}
				>
					{m.mods_nexus_handler_register()}
				</Button>
			{/if}
			{#if otherHandlerRefusal}
				<p class="text-error-400 text-sm" role="alert">{nexusErrorText(otherHandlerRefusal)}</p>
			{/if}
		</div>
	{/if}
</Card>
