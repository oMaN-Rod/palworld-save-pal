<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button } from '$components/ui';
	import { getModsState } from '$states';
	import { MessageType } from '$types';
	import type { LibraryMod, RecordedModError } from '$types';
	import * as m from '$i18n/messages';
	import {
		applyOffers,
		deployedText,
		displayName,
		holderText,
		inUseDeployed,
		inUseFrameworks,
		inUseHolders,
		releasable,
		type UsageLabels
	} from './modList';

	let {
		mod,
		error,
		labels,
		onRetry
	}: {
		mod: LibraryMod;
		error: RecordedModError;
		labels: UsageLabels;
		onRetry: () => void;
	} = $props();

	const modsState = getModsState();
	const name = $derived(displayName(mod));
	const holders = $derived(inUseHolders(error));
	const deployed = $derived(inUseDeployed(error));
	const frameworks = $derived(inUseFrameworks(error));
	const offers = $derived(applyOffers(error));
	const canRelease = $derived(releasable(error));
	const hasEnabled = $derived(holders.some((holder) => holder.enabled));
	const released = $derived(modsState.lastRelease[mod.id]);
	const releaseError = $derived(
		modsState.lastErrorFor(MessageType.MOD_RELEASE_PROFILES, undefined, { mod_id: mod.id })
	);
</script>

<div class="flex flex-col gap-1">
	<p class="text-error-400 flex items-start gap-1.5" role="alert">
		<Icon icon="tabler:alert-circle" size={12} class="mt-0.5 shrink-0" />
		{m.mods_in_use_title({ name })}
	</p>
	{#if holders.length > 0}
		<ul class="text-surface-300 flex flex-col gap-0.5 pl-4.5">
			{#each holders as holder (holder.profile_id)}
				<li>{holderText(holder, labels)}</li>
			{/each}
		</ul>
	{/if}
	{#if deployed.length > 0}
		<p class="text-surface-300 pl-4.5">
			{m.mods_in_use_deployed({
				targets: deployed.map((entry) => deployedText(entry, labels)).join(', ')
			})}
		</p>
	{/if}
	{#if frameworks.length > 0}
		<p class="text-surface-300 pl-4.5">
			{m.mods_list_in_use_frameworks({
				targets: frameworks.map((entry) => deployedText(entry, labels)).join(', ')
			})}
		</p>
	{/if}
	{#if hasEnabled}
		<p class="text-surface-400 pl-4.5">{m.mods_in_use_enabled_hint()}</p>
	{/if}
	{#if released}
		<p class="text-surface-300 pl-4.5">
			{m.mods_in_use_released({ count: released.removed.length })}
		</p>
	{/if}
	{#if releaseError}
		<p class="text-error-400 pl-4.5" role="alert">{releaseError.message}</p>
	{/if}
	<div class="flex flex-wrap gap-1.5 pl-4.5">
		{#if canRelease}
			<Button
				size="sm"
				variant="ghost"
				aria-label={m.mods_in_use_release_named({ name })}
				disabled={modsState.releasing[mod.id] ?? false}
				onclick={() => modsState.releaseProfiles(mod.id)}
			>
				{m.mods_in_use_release()}
			</Button>
		{/if}
		{#each offers as offer (offer.target_id)}
			{@const target = deployedText(offer, labels)}
			<Button
				size="sm"
				variant="ghost"
				aria-label={m.mods_in_use_apply_named({ target, name })}
				disabled={modsState.applying[offer.target_id] ?? false}
				onclick={() => modsState.apply(offer.target_id)}
			>
				{m.mods_in_use_apply({ target })}
			</Button>
		{/each}
		<Button
			size="sm"
			variant="ghost"
			aria-label={m.mods_in_use_retry_named({ name })}
			onclick={onRetry}
		>
			{m.mods_in_use_retry()}
		</Button>
	</div>
</div>
