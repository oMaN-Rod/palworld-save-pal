<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import * as m from '$i18n/messages';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Input, Spinner } from '$components/ui';
	import { getServerState } from '$states';
	import { send, sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import {
		type BrowseEntry,
		type BrowseFrame,
		type GamepassSaveEntry,
		type LocalSaveEntry,
		type PickerSection,
		hasLevelSav,
		isServerRunning,
		levelSavPath,
		popToIndex,
		pushFrame,
		replyError,
		sortGamepassSaves,
		sortLocalSaves
	} from '$lib/components/signal/pickerModel';

	let { section }: { section: PickerSection } = $props();

	const serverState = getServerState();

	let localSaves = $state<LocalSaveEntry[]>([]);
	let localLoading = $state(true);
	let localError = $state('');

	let gamepassSaves = $state<GamepassSaveEntry[]>([]);
	let gamepassLoading = $state(true);
	let gamepassError = $state('');

	let browseStack = $state<BrowseFrame[]>([{ path: '', entries: [] }]);
	let browseLoading = $state(true);
	let browseError = $state('');
	let browsePathInput = $state('');

	const servers = $derived(serverState.servers);
	const currentBrowseFrame = $derived(browseStack[browseStack.length - 1]);

	function formatModified(ms: number): string {
		return ms > 0 ? new Date(ms).toLocaleString() : '';
	}

	async function loadLocalSaves() {
		localLoading = true;
		localError = '';
		try {
			const result = await sendAndWait<{ saves?: LocalSaveEntry[]; error?: string }>(
				MessageType.LIST_LOCAL_SAVES
			);
			const error = replyError(result);
			if (error) {
				localError = error;
			} else {
				localSaves = sortLocalSaves(
					(result.saves ?? []).filter((save) => save.save_type !== 'gamepass')
				);
			}
		} catch (err: any) {
			localError = err?.message ?? String(err);
		} finally {
			localLoading = false;
		}
	}

	async function loadGamepassSaves() {
		gamepassLoading = true;
		gamepassError = '';
		try {
			const result = await sendAndWait<{
				saves?: Record<string, GamepassSaveEntry>;
				error?: string;
			}>(MessageType.SCAN_GAMEPASS_SAVES);
			const error = replyError(result);
			if (error) {
				gamepassError = error;
			} else {
				gamepassSaves = sortGamepassSaves(Object.values(result.saves ?? {}));
			}
		} catch (err: any) {
			gamepassError = err?.message ?? String(err);
		} finally {
			gamepassLoading = false;
		}
	}

	async function fetchBrowseFrame(path: string): Promise<BrowseFrame> {
		const result = await sendAndWait<{ path?: string; entries?: BrowseEntry[]; error?: string }>(
			MessageType.BROWSE_DIRECTORY,
			{ path }
		);
		const error = replyError(result);
		if (error) throw new Error(error);
		return { path: result.path ?? '', entries: result.entries ?? [] };
	}

	async function loadBrowseRoot() {
		browseLoading = true;
		browseError = '';
		try {
			browseStack = [await fetchBrowseFrame('')];
		} catch (err: any) {
			browseError = err?.message ?? String(err);
		} finally {
			browseLoading = false;
		}
	}

	async function enterDirectory(entry: BrowseEntry) {
		if (!entry.is_dir) return;
		browseLoading = true;
		browseError = '';
		try {
			browseStack = pushFrame(browseStack, await fetchBrowseFrame(entry.path));
		} catch (err: any) {
			browseError = err?.message ?? String(err);
		} finally {
			browseLoading = false;
		}
	}

	async function goToBrowsePath(event: SubmitEvent) {
		event.preventDefault();
		const path = browsePathInput.trim();
		if (!path) return;
		browseLoading = true;
		browseError = '';
		try {
			browseStack = [await fetchBrowseFrame(path)];
			browsePathInput = '';
		} catch (err: any) {
			browseError = err?.message ?? String(err);
		} finally {
			browseLoading = false;
		}
	}

	function goToBreadcrumb(index: number) {
		browseStack = popToIndex(browseStack, index);
	}

	async function loadSteamSave(path: string) {
		await goto('/loading');
		send(MessageType.SELECT_SAVE, { type: 'steam', path, local: false });
	}

	async function loadGamepassSave(saveId: string) {
		await goto('/loading');
		send(MessageType.SELECT_GAMEPASS_SAVE, saveId);
	}

	async function loadServerSave(serverId: number) {
		await serverState.loadServerSave(serverId);
		await goto('/edit');
	}

	async function loadFromBrowse() {
		const path = levelSavPath(currentBrowseFrame.entries);
		if (!path) return;
		await loadSteamSave(path);
	}

	onMount(() => {
		loadLocalSaves();
		loadGamepassSaves();
		serverState.loadServers();
		loadBrowseRoot();
	});
</script>

{#snippet loading()}
	<div class="flex items-center gap-2 py-4">
		<Spinner size="size-6" />
		<span class="text-surface-300 text-sm">{m.loading()}</span>
	</div>
{/snippet}

{#if section === 'steam'}
	{#if localLoading}
		{@render loading()}
	{:else if localError}
		<p class="text-error-400 text-sm">{m.error()}: {localError}</p>
	{:else if localSaves.length === 0}
		<p class="text-surface-400 text-sm">{m.remote_saves_empty()}</p>
	{:else}
		<div class="flex flex-col gap-2">
			{#each localSaves as save (save.path)}
				{@const modified = formatModified(save.modified_ms)}
				<button
					type="button"
					class="bg-surface-800 hover:bg-surface-700 flex w-full items-center justify-between gap-3 rounded-sm p-3 text-left"
					onclick={() => loadSteamSave(save.path)}
				>
					<div class="min-w-0">
						<p class="truncate font-medium">{save.name}</p>
						<p class="text-surface-400 truncate text-xs">{save.path}</p>
						{#if save.mod_profile}
							<p class="text-primary-400 truncate text-xs">
								{m.mods_worlds_linked({ name: save.mod_profile.profile_name })}
							</p>
						{/if}
						{#if modified}
							<p class="text-surface-500 text-xs">{modified}</p>
						{/if}
					</div>
					<Icon icon="tabler:chevron-right" size={16} class="text-surface-400 shrink-0" />
				</button>
			{/each}
		</div>
	{/if}
{:else if section === 'gamepass'}
	{#if gamepassLoading}
		{@render loading()}
	{:else if gamepassError}
		<p class="text-error-400 text-sm">{m.error()}: {gamepassError}</p>
	{:else if gamepassSaves.length === 0}
		<p class="text-surface-400 text-sm">{m.remote_saves_empty()}</p>
	{:else}
		<div class="flex flex-col gap-2">
			{#each gamepassSaves as save (save.save_id)}
				{@const modified = formatModified(save.last_modified * 1000)}
				<button
					type="button"
					class="bg-surface-800 hover:bg-surface-700 flex w-full items-center justify-between gap-3 rounded-sm p-3 text-left"
					onclick={() => loadGamepassSave(save.save_id)}
				>
					<div class="min-w-0">
						<p class="truncate font-medium">{save.world_name}</p>
						<div class="text-surface-400 flex items-center gap-1 text-xs">
							<Icon icon="tabler:users" size={14} />
							<span>{save.player_count}</span>
						</div>
						{#if modified}
							<p class="text-surface-500 text-xs">{modified}</p>
						{/if}
					</div>
					<Icon icon="tabler:chevron-right" size={16} class="text-surface-400 shrink-0" />
				</button>
			{/each}
		</div>
	{/if}
{:else if section === 'server'}
	{#if serverState.loading}
		{@render loading()}
	{:else if servers.length === 0}
		<p class="text-surface-400 text-sm">{m.remote_saves_empty()}</p>
	{:else}
		<div class="flex flex-col gap-2">
			{#each servers as server (server.id)}
				{@const running = isServerRunning(server)}
				<button
					type="button"
					class="bg-surface-800 hover:bg-surface-700 disabled:hover:bg-surface-800 flex w-full items-center justify-between gap-3 rounded-sm p-3 text-left disabled:cursor-not-allowed disabled:opacity-50"
					disabled={running}
					onclick={() => loadServerSave(server.id)}
				>
					<div class="min-w-0">
						<p class="truncate font-medium">{server.name}</p>
						<p class="text-surface-400 truncate text-xs">{server.saves_path}</p>
						{#if running}
							<p class="text-warning-400 text-xs">{m.remote_saves_server_running()}</p>
						{/if}
					</div>
					<Icon icon="tabler:chevron-right" size={16} class="text-surface-400 shrink-0" />
				</button>
			{/each}
		</div>
	{/if}
{:else}
	<div class="mb-3 flex flex-wrap items-center gap-1 text-sm">
		{#each browseStack as frame, index (index)}
			{#if index > 0}
				<Icon icon="tabler:chevron-right" size={14} class="text-surface-500" />
			{/if}
			<button
				type="button"
				class="text-surface-300 hover:text-surface-50 max-w-[10rem] truncate underline-offset-2 hover:underline"
				disabled={index === browseStack.length - 1}
				onclick={() => goToBreadcrumb(index)}
			>
				{frame.path || m.signal_desktop_fallback_name()}
			</button>
		{/each}
	</div>

	<form class="mb-3 flex items-center gap-2" onsubmit={goToBrowsePath}>
		<div class="grow">
			<Input bind:value={browsePathInput} aria-label={m.remote_saves_browse()} />
		</div>
		<Button type="submit" variant="secondary" size="sm">{m.remote_saves_go()}</Button>
	</form>

	{#if browseLoading}
		{@render loading()}
	{:else if browseError}
		<p class="text-error-400 text-sm">{m.error()}: {browseError}</p>
	{:else if currentBrowseFrame.entries.length === 0}
		<p class="text-surface-400 text-sm">{m.remote_saves_empty()}</p>
	{:else}
		<div class="flex flex-col gap-1">
			{#each currentBrowseFrame.entries as entry (entry.path)}
				<button
					type="button"
					class="hover:bg-surface-700 flex w-full items-center gap-2 rounded-sm p-2 text-left text-sm disabled:opacity-50"
					disabled={!entry.is_dir}
					onclick={() => enterDirectory(entry)}
				>
					<Icon
						icon={entry.is_dir ? 'tabler:folder' : 'tabler:file'}
						size={16}
						class="text-surface-400 shrink-0"
					/>
					<span class="truncate">{entry.name}</span>
				</button>
			{/each}
		</div>
	{/if}

	<div class="mt-3 flex justify-end">
		<Button
			variant="primary"
			disabled={!hasLevelSav(currentBrowseFrame.entries)}
			onclick={loadFromBrowse}
		>
			{m.remote_saves_load()}
		</Button>
	</div>
{/if}
