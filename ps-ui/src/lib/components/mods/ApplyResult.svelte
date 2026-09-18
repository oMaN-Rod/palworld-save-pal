<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card } from '$components/ui';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { getModalState, getModsState } from '$states';
	import { cn } from '$theme';
	import type { ApplyResult as ApplyReply } from '$types';
	import * as m from '$i18n/messages';
	import type { ModsTab } from './TargetPanel.svelte';
	import { confirmReplace, errorText, isFailed, isNotable, stringList } from './applyOutcome';

	let {
		result,
		targetId,
		onShowTab,
		onDismiss,
		onAction,
		framed = true
	}: {
		result: ApplyReply;
		targetId: string;
		onShowTab: (tab: ModsTab) => void;
		onDismiss?: () => void;
		/** Called after an action that applies again, so a surrounding modal can close. */
		onAction?: () => void;
		framed?: boolean;
	} = $props();

	const modsState = getModsState();
	const modal = getModalState();
	const remoteMode = getRemoteMode();

	const error = $derived(result.error);
	const code = $derived(error?.code);
	const failed = $derived(isFailed(result));
	const occupants = $derived(code === 'unmanaged_occupant' ? stringList(error?.paths) : []);
	const moved = $derived(code === 'replace_partial' ? stringList(error?.moved) : []);
	const cause = $derived.by(() => {
		if (code !== 'replace_partial' || typeof error?.cause !== 'string') return '';
		return errorText({ code: error.cause, message: '' }, modsState.mods);
	});
	const showBackups = $derived(code === 'replace_partial' || result.backup_dir !== null);
	const showApplyAgain = $derived(code === 'replace_partial' || result.mid_apply);
	const notable = $derived(isNotable(result));

	const reasonText = {
		unreadable: m.mods_apply_unreadable,
		drift: m.mods_apply_drift
	};

	function adopt() {
		modsState.scan(targetId);
		onShowTab('scan');
	}

	async function replace(paths: string[]) {
		if (!(await confirmReplace(modal, remoteMode.active))) return;
		modsState.apply(targetId, paths);
		onAction?.();
	}

	function applyAgain() {
		modsState.apply(targetId);
		onAction?.();
	}
</script>

{#snippet pathList(paths: string[])}
	<ul class="flex flex-col gap-0.5 font-mono text-xs">
		{#each paths as path (path)}
			<li class="truncate" title={path}>{path}</li>
		{/each}
	</ul>
{/snippet}

{#snippet heading(text: string)}
	<p class="text-surface-300 text-xs font-medium">{text}</p>
{/snippet}

{#snippet outcome()}
	{#if onDismiss}
		<Button
			variant="ghost"
			size="sm"
			class="absolute top-2 right-2"
			aria-label={m.mods_apply_dismiss()}
			onclick={onDismiss}
		>
			<Icon icon="tabler:x" size={14} />
		</Button>
	{/if}

	{#if error}
		<div class="flex flex-col gap-2">
			<p class="text-error-400 flex items-start gap-2">
				<Icon icon="tabler:alert-circle" size={14} class="mt-0.5 shrink-0" />
				<span>{errorText(error, modsState.mods)}</span>
			</p>
			{#if occupants.length > 0}
				{@render pathList(occupants)}
			{/if}
			{#if moved.length > 0}
				{@render pathList(moved)}
			{/if}
			{#if cause}
				<p class="text-surface-300">{cause}</p>
			{/if}
		</div>
	{/if}

	{#if result.mid_apply}
		<p class="text-error-400 flex items-center gap-2">
			<Icon icon="tabler:alert-triangle" size={14} />
			{m.mods_apply_mid_apply()}
		</p>
	{/if}

	{#if result.needs_attention.length > 0}
		<div class="flex flex-col gap-1">
			{@render heading(m.mods_panel_needs_attention())}
			<ul class="flex flex-col gap-1.5">
				{#each result.needs_attention as entry (entry.path)}
					<li class="flex flex-col">
						<span class="truncate font-mono text-xs" title={entry.path}>{entry.path}</span>
						<span class="text-surface-400 text-xs">
							{reasonText[entry.reason]?.() ?? entry.reason}
						</span>
					</li>
				{/each}
			</ul>
		</div>
	{/if}

	{#if result.preserved.length > 0}
		<div class="flex flex-col gap-1">
			{@render heading(m.mods_apply_preserved())}
			{@render pathList(result.preserved)}
		</div>
	{/if}

	{#if result.new_copies.length > 0}
		<div class="flex flex-col gap-1">
			{@render heading(m.mods_apply_new_copies())}
			{@render pathList(result.new_copies)}
		</div>
	{/if}

	{#if result.skipped_new_copies.length > 0}
		<div class="flex flex-col gap-1">
			{@render heading(m.mods_apply_skipped_new_copies())}
			{@render pathList(result.skipped_new_copies)}
		</div>
	{/if}

	{#if result.backup_dir !== null && code !== 'replace_partial'}
		<p class="text-surface-300 flex items-center gap-2">
			<Icon icon="tabler:archive" size={14} />
			{m.mods_apply_backed_up()}
		</p>
	{/if}

	{#if occupants.length > 0 || showBackups || showApplyAgain}
		<div class="flex flex-wrap gap-2">
			{#if occupants.length > 0}
				<Button size="sm" onclick={adopt}>{m.mods_apply_adopt()}</Button>
				<Button size="sm" variant="ghost" onclick={() => replace(occupants)}>
					{m.mods_apply_replace()}
				</Button>
			{/if}
			{#if showApplyAgain}
				<Button size="sm" variant="ghost" onclick={applyAgain}>
					{m.mods_apply_again()}
				</Button>
			{/if}
			{#if showBackups}
				<Button size="sm" variant="ghost" onclick={() => onShowTab('backups')}>
					{m.mods_apply_view_backups()}
				</Button>
			{/if}
		</div>
	{/if}
{/snippet}

{#if notable}
	<div role={failed ? 'alert' : 'status'}>
		{#if framed}
			<Card
				padding="p-3"
				class={cn(
					'relative flex flex-col gap-3 border text-sm',
					onDismiss && 'pr-10',
					failed ? 'border-error-500/40 bg-error-500/10' : 'border-warning-500/40 bg-warning-500/10'
				)}
			>
				{@render outcome()}
			</Card>
		{:else}
			<div class="flex flex-col gap-3 text-sm">
				{@render outcome()}
			</div>
		{/if}
	</div>
{/if}
