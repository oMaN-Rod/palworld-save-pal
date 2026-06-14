<script lang="ts">
	import type { GamepassSave, GamepassContainerInfo } from '$types';
	import { MessageType } from '$types';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { getToastState } from '$states';
	import {
		Users,
		HardDrive,
		Clock,
		ChevronDown,
		ChevronRight,
		Layers,
		FileBox,
		Trash2,
		Pencil
	} from 'lucide-svelte';
	import { cn } from '$theme';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';

	let {
		saves = $bindable(),
		selectable = false,
		manageable = false,
		onselect = (_save: GamepassSave) => {},
		onchange = () => {}
	}: {
		saves: Record<string, GamepassSave>;
		selectable?: boolean;
		manageable?: boolean;
		onselect?: (save: GamepassSave) => void;
		onchange?: () => void;
	} = $props();

	const toast = getToastState();

	let expandedSaves: Set<string> = $state(new Set());
	let selectedSaveId: string | null = $state(null);
	let confirmDelete: string | null = $state(null);
	let renamingSaveId: string | null = $state(null);
	let renameValue = $state('');

	const saveList = $derived(
		Object.values(saves).sort((a, b) => b.last_modified - a.last_modified)
	);

	function formatDate(timestamp: number): string {
		if (!timestamp) return 'Unknown';
		return new Date(timestamp * 1000).toLocaleString();
	}

	function formatSize(bytes: number): string {
		if (bytes === 0) return '0 B';
		const units = ['B', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(1024));
		return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
	}

	function formatRelativeTime(timestamp: number): string {
		if (!timestamp) return '';
		const now = Date.now() / 1000;
		const diff = now - timestamp;
		if (diff < 60) return 'just now';
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		return `${Math.floor(diff / 86400)}d ago`;
	}

	function toggleExpanded(saveId: string) {
		const next = new Set(expandedSaves);
		if (next.has(saveId)) {
			next.delete(saveId);
		} else {
			next.add(saveId);
		}
		expandedSaves = next;
	}

	function handleSelect(save: GamepassSave) {
		if (!selectable) return;
		selectedSaveId = save.save_id;
		onselect(save);
	}

	function getContainerGroups(
		containers: GamepassContainerInfo[]
	): Record<string, GamepassContainerInfo[]> {
		const groups: Record<string, GamepassContainerInfo[]> = {};
		for (const c of containers) {
			let groupKey = c.container_type;
			if (groupKey.startsWith('Players-')) {
				groupKey = 'Players';
			}
			if (!groups[groupKey]) {
				groups[groupKey] = [];
			}
			groups[groupKey].push(c);
		}
		for (const key of Object.keys(groups)) {
			groups[key].sort((a, b) => b.seq - a.seq || b.last_modified - a.last_modified);
		}
		return groups;
	}

	function getContainerTypeLabel(type: string): string {
		if (type === 'Level') return 'World Data';
		if (type === 'LevelMeta') return 'World Metadata';
		if (type === 'LocalData') return 'Local Data';
		if (type === 'WorldOption') return 'World Options';
		if (type === 'Players') return 'Player Saves';
		return type;
	}

	async function handleDeleteSave(saveId: string) {
		try {
			const result = await sendAndWait<{ message?: string; error?: string }>(
				MessageType.DELETE_GAMEPASS_SAVE,
				{ save_id: saveId }
			);
			if (result.error) {
				toast.add(result.error, 'Error', 'error');
			} else {
				toast.add(result.message ?? 'Save deleted');
				delete saves[saveId];
				saves = { ...saves };
				onchange();
			}
		} catch (err: any) {
			toast.add(`Failed: ${err.message}`, 'Error', 'error');
		}
		confirmDelete = null;
	}

	async function handleRename(saveId: string) {
		if (!renameValue.trim()) return;
		try {
			const result = await sendAndWait<{ message?: string; error?: string }>(
				MessageType.RENAME_GAMEPASS_WORLD,
				{ save_id: saveId, new_name: renameValue.trim() }
			);
			if (result.error) {
				toast.add(result.error, 'Error', 'error');
			} else {
				toast.add(result.message ?? 'Renamed');
				if (saves[saveId]) {
					saves[saveId].world_name = renameValue.trim();
					saves = { ...saves };
				}
				onchange();
			}
		} catch (err: any) {
			toast.add(`Failed: ${err.message}`, 'Error', 'error');
		}
		renamingSaveId = null;
	}

	function startRename(save: GamepassSave) {
		renamingSaveId = save.save_id;
		renameValue = save.world_name;
	}
</script>

<div class="flex flex-col gap-2">
	<h2 class="text-surface-100 text-lg font-bold">
		{m.available_entity({ entity: c.saves })}
		<span class="text-surface-400 text-sm font-normal">({saveList.length})</span>
	</h2>

	{#if saveList.length === 0}
		<p class="text-surface-400 text-sm">
			No GamePass saves found. Make sure you have Palworld installed via GamePass and have
			created at least one world.
		</p>
	{:else}
		<div class="bg-surface-800 flex max-h-[600px] flex-col overflow-y-auto rounded-lg">
			{#each saveList as save (save.save_id)}
				{@const isExpanded = expandedSaves.has(save.save_id)}
				{@const isSelected = selectedSaveId === save.save_id}
				{@const groups = getContainerGroups(save.containers)}

				<div
					class={cn(
						'border-surface-700 border-b transition-colors',
						isSelected && 'bg-secondary-500/20'
					)}
				>
					<div class="flex items-center gap-2 p-3">
						<!-- Expand toggle -->
						<button
							class="text-surface-400 hover:text-surface-200 shrink-0"
							onclick={() => toggleExpanded(save.save_id)}
						>
							{#if isExpanded}
								<ChevronDown size={16} />
							{:else}
								<ChevronRight size={16} />
							{/if}
						</button>

						<!-- Save info: either rename form or display -->
						{#if renamingSaveId === save.save_id}
							<!-- Rename form (not inside a button) -->
							<!-- svelte-ignore a11y_autofocus -->
							<form
								class="flex min-w-0 grow items-center gap-2"
								onsubmit={(e) => {
									e.preventDefault();
									handleRename(save.save_id);
								}}
							>
								<input
									type="text"
									bind:value={renameValue}
									class="bg-surface-900 border-surface-600 min-w-0 grow rounded border px-2 py-1 text-sm text-surface-50"
									autofocus
									onkeydown={(e) => {
										if (e.key === 'Escape') renamingSaveId = null;
									}}
								/>
								<button
									type="submit"
									class="text-primary-400 hover:text-primary-300 shrink-0 text-xs"
								>
									Save
								</button>
								<button
									type="button"
									class="text-surface-400 hover:text-surface-200 shrink-0 text-xs"
									onclick={() => (renamingSaveId = null)}
								>
									Cancel
								</button>
							</form>
						{:else}
							<!-- Normal display (clickable in select mode) -->
							<button
								class={cn(
									'flex min-w-0 grow items-center gap-3 text-left',
									selectable && 'hover:text-primary-400 cursor-pointer'
								)}
								onclick={() => handleSelect(save)}
								disabled={!selectable}
							>
								<div class="flex min-w-0 grow flex-col">
									<span class="truncate font-semibold text-surface-50">
										{save.world_name}
									</span>
									<span class="text-surface-400 truncate text-xs">
										{save.save_id}
									</span>
								</div>
							</button>

							<!-- Stats -->
							<div
								class="text-surface-300 flex shrink-0 items-center gap-4 text-sm"
							>
								<div class="flex items-center gap-1" title="Players">
									<Users size={14} />
									<span>{save.player_count}</span>
								</div>
								<div class="flex items-center gap-1" title="Total Size">
									<HardDrive size={14} />
									<span>{formatSize(save.total_size)}</span>
								</div>
								<div
									class="flex items-center gap-1"
									title={formatDate(save.last_modified)}
								>
									<Clock size={14} />
									<span>{formatRelativeTime(save.last_modified)}</span>
								</div>
							</div>

							<!-- Management actions -->
							{#if manageable}
								<div class="flex shrink-0 items-center gap-1">
									<button
										class="text-surface-400 hover:text-primary-400 rounded p-1"
										title="Rename world"
										onclick={() => startRename(save)}
									>
										<Pencil size={14} />
									</button>
									{#if confirmDelete === save.save_id}
										<span class="text-xs text-red-400">Delete?</span>
										<button
											class="rounded bg-red-600 px-2 py-0.5 text-xs text-white hover:bg-red-500"
											onclick={() => handleDeleteSave(save.save_id)}
										>
											Yes
										</button>
										<button
											class="text-surface-400 hover:text-surface-200 text-xs"
											onclick={() => (confirmDelete = null)}
										>
											No
										</button>
									{:else}
										<button
											class="text-surface-400 hover:text-red-400 rounded p-1"
											title="Delete save"
											onclick={() => (confirmDelete = save.save_id)}
										>
											<Trash2 size={14} />
										</button>
									{/if}
								</div>
							{/if}
						{/if}
					</div>

					<!-- Expanded container details -->
					{#if isExpanded}
						<div class="bg-surface-900/50 border-surface-700 border-t px-4 py-2">
							<div
								class="text-surface-400 mb-1 text-xs font-semibold uppercase tracking-wider"
							>
								Containers
							</div>
							{#each Object.entries(groups) as [groupType, groupContainers]}
								<div class="mb-2">
									<div class="flex items-center gap-2 py-1">
										{#if groupType === 'Players'}
											<Users size={12} class="text-surface-400" />
										{:else}
											<FileBox size={12} class="text-surface-400" />
										{/if}
										<span class="text-surface-200 text-sm font-medium">
											{getContainerTypeLabel(groupType)}
										</span>
										{#if groupContainers.length > 1}
											<span class="text-surface-500 text-xs">
												({groupContainers.length})
											</span>
										{/if}
									</div>
									<div class="ml-5 flex flex-col gap-0.5">
										{#each groupContainers as container}
											<div
												class="text-surface-400 flex items-center gap-3 text-xs"
											>
												<span
													class="text-surface-300 min-w-0 truncate font-mono"
												>
													{container.container_type}
												</span>
												<div class="flex items-center gap-1">
													<Layers size={10} />
													<span>v{container.seq}</span>
												</div>
												<span>{formatSize(container.size)}</span>
												<span
													class="text-surface-500"
													title={formatDate(container.last_modified)}
												>
													{formatRelativeTime(container.last_modified)}
												</span>
											</div>
										{/each}
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
