<script module lang="ts">
	/** Set names start with a UTC `YYYYMMDD-HHMMSS` stamp. */
	export function backupDate(name: string): Date | null {
		const match = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})/.exec(name);
		if (!match) return null;
		const [year, month, day, hour, minute, second] = match.slice(1).map(Number);
		const date = new Date(Date.UTC(year, month - 1, day, hour, minute, second));
		const exact =
			date.getUTCMonth() === month - 1 &&
			date.getUTCDate() === day &&
			date.getUTCHours() === hour &&
			date.getUTCMinutes() === minute &&
			date.getUTCSeconds() === second;
		return exact ? date : null;
	}
</script>

<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { untrack } from 'svelte';
	import { Button, Card } from '$components/ui';
	import { getModalState, getModsState } from '$states';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { MessageType } from '$types';
	import type { BackupSet, ModError } from '$types';
	import * as m from '$i18n/messages';
	import { formatSize } from './modList';

	let { targetId }: { targetId: string } = $props();

	const modsState = getModsState();
	const modal = getModalState();
	const remoteMode = getRemoteMode();

	const sets = $derived(modsState.backups[targetId]);
	const restoring = $derived(modsState.restoring[targetId] ?? false);
	const deleting = $derived(modsState.deleting[targetId] ?? false);
	const restore = $derived(modsState.lastRestore[targetId]);

	let expanded = $state<Record<string, boolean>>({});
	let confirming = $state(false);

	const refusal = $derived.by(() => {
		const types = [
			MessageType.MOD_BACKUP_LIST,
			MessageType.MOD_BACKUP_RESTORE,
			MessageType.MOD_BACKUP_DELETE
		];
		for (const type of types) {
			if (type === MessageType.MOD_BACKUP_RESTORE && restoring) continue;
			if (type === MessageType.MOD_BACKUP_DELETE && deleting) continue;
			const error = modsState.lastErrorFor(type, targetId);
			if (error) return error;
		}
		return undefined;
	});

	const skipReasons: Record<string, () => string> = {
		occupied: m.mods_backups_skip_occupied,
		changed: m.mods_backups_skip_changed,
		invalid_backup_key: m.mods_backups_skip_invalid_key,
		outside_target: m.mods_backups_skip_outside_target
	};

	function setLabel(name: string): string {
		return backupDate(name)?.toLocaleString() ?? name;
	}

	function refusalText(error: ModError): string {
		switch (error.code) {
			case 'apply_in_progress':
				return m.mods_backups_apply_running();
			case 'journal_open':
				return m.mods_backups_journal_open();
			case 'target_locked':
				return m.mods_backups_target_locked();
			default:
				return error.message;
		}
	}

	async function remove(set: BackupSet) {
		if (confirming) return;
		confirming = true;
		let confirmed = false;
		try {
			confirmed = await modal.showConfirmModal({
				title: m.mods_backups_delete_title(),
				message: m.mods_backups_delete_message({ date: setLabel(set.name) }),
				confirmText: m.mods_backups_delete(),
				cancelText: m.mods_panel_cancel()
			});
		} finally {
			confirming = false;
		}
		if (confirmed && !remoteMode.active) modsState.deleteBackup(targetId, set.name);
	}

	$effect(() => {
		const id = targetId;
		void modsState.resets;
		if (remoteMode.active) return;
		untrack(() => modsState.loadBackups(id));
	});
</script>

{#if remoteMode.active}
	<Card padding="p-6" class="text-surface-400 flex items-center gap-3 text-sm">
		<Icon icon="tabler:device-desktop" size={20} class="shrink-0" />
		{m.mods_backups_remote()}
	</Card>
{:else}
	<div class="flex flex-col gap-4">
		{#if refusal}
			<p class="text-error-400 flex items-start gap-2 text-sm" role="alert">
				<Icon icon="tabler:alert-circle" size={14} class="mt-0.5 shrink-0" />
				<span>{refusalText(refusal)}</span>
			</p>
		{/if}

		{#if restore && !restoring}
			<Card padding="p-3" class="flex flex-col gap-2 text-sm">
				<p>
					{m.mods_backups_restored({ date: setLabel(restore.set), count: restore.restored.length })}
				</p>
				{#if restore.skipped.length > 0}
					<p class="text-warning-400 text-xs font-medium">{m.mods_backups_skipped()}</p>
					<ul class="flex flex-col gap-1.5">
						{#each restore.skipped as skip (skip.path)}
							<li class="flex flex-col">
								<span class="truncate font-mono text-xs" title={skip.path}>{skip.path}</span>
								<span class="text-surface-400 text-xs">
									{skipReasons[skip.reason]?.() ?? skip.reason}
								</span>
							</li>
						{/each}
					</ul>
				{/if}
			</Card>
		{/if}

		{#if sets && sets.length === 0}
			<Card padding="p-6" class="text-surface-400 flex items-center gap-3 text-sm">
				<Icon icon="tabler:archive" size={20} class="shrink-0" />
				{m.mods_backups_empty()}
			</Card>
		{:else if sets}
			<ul class="divide-surface-700 divide-y">
				{#each sets as set (set.name)}
					<li class="flex flex-col gap-2 py-3">
						<div class="flex flex-wrap items-center justify-between gap-3">
							<div class="flex min-w-0 flex-col">
								<span class="truncate font-medium" title={set.name}>{setLabel(set.name)}</span>
								<span class="text-surface-400 flex gap-3 text-xs">
									<span>{formatSize(set.size_bytes)}</span>
									<span>{m.mods_file_count({ count: set.entries.length })}</span>
								</span>
							</div>
							<div class="flex shrink-0 flex-wrap gap-2">
								<Button
									size="sm"
									variant="ghost"
									aria-expanded={expanded[set.name] ?? false}
									onclick={() => (expanded = { ...expanded, [set.name]: !expanded[set.name] })}
								>
									{expanded[set.name] ? m.mods_backups_hide_files() : m.mods_backups_show_files()}
								</Button>
								<Button
									size="sm"
									disabled={restoring}
									onclick={() => modsState.restoreBackup(targetId, set.name)}
								>
									{m.mods_backups_restore_all()}
								</Button>
								<Button
									size="sm"
									variant="ghost"
									disabled={restoring || deleting || confirming}
									onclick={() => remove(set)}
								>
									<Icon icon="tabler:trash" size={14} class="text-red-400" />
									{m.mods_backups_delete()}
								</Button>
							</div>
						</div>
						{#if expanded[set.name]}
							<ul class="flex flex-col gap-1">
								{#each set.entries as entry (entry.backup_key)}
									<li class="flex items-center justify-between gap-3">
										<span class="truncate font-mono text-xs" title={entry.original_path}>
											{entry.original_path}
										</span>
										<Button
											size="sm"
											variant="ghost"
											class="shrink-0"
											disabled={restoring}
											onclick={() =>
												modsState.restoreBackup(targetId, set.name, [entry.original_path])}
										>
											{m.mods_backups_restore()}
										</Button>
									</li>
								{/each}
							</ul>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}
