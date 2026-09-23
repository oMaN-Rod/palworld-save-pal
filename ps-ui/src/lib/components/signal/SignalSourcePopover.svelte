<script lang="ts">
	import { onMount } from 'svelte';
	import { Input, Popover, Select } from '$components/ui';
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import * as m from '$i18n/messages';
	import { getServerState, getSignalState } from '$states';
	import type { SelectOption, SelectSignalSourceKind } from '$types';
	import { buildSetSourceRequest, sourceKindFromStatus } from './signalSource.utils';

	const signalState = getSignalState();
	const serverState = getServerState();

	type SourceKind = SelectSignalSourceKind;

	let sourceKind = $state<SourceKind>('off');
	let filePath = $state('');
	let selectedServerId = $state<number | ''>('');
	let sourceSaving = $state(false);
	let sourceError = $state<string | null>(null);
	let sourceSynced = false;

	const status = $derived(signalState.status);

	const serverOptions = $derived(
		serverState.servers.map((server): SelectOption => ({ value: server.id, label: server.name }))
	);

	$effect(() => {
		if (sourceSynced || !status) return;
		sourceSynced = true;
		sourceKind = sourceKindFromStatus(status.source.kind);
	});

	onMount(() => {
		serverState.loadServers();
	});

	async function applySource(kind: SourceKind) {
		sourceKind = kind;
		sourceError = null;
		sourceSaving = true;
		try {
			const request = buildSetSourceRequest(kind, { filePath, selectedServerId });
			if (request) await signalState.setSource(request);
		} catch (error) {
			sourceError = error instanceof Error ? error.message : String(error);
		} finally {
			sourceSaving = false;
		}
	}

	function handleFilePathCommit() {
		if (sourceKind === 'file') void applySource('file');
	}

	function handleServerChange() {
		if (sourceKind === 'server' && selectedServerId !== '') void applySource('server');
	}
</script>

<Popover position="bottom-end" popoverClass="w-80">
	<button
		type="button"
		id="signal-source-button"
		class="tap-target bg-surface-800 hover:bg-surface-700 flex items-center gap-1.5 rounded-sm px-3 py-1.5 text-sm font-medium"
	>
		<Icon icon="tabler:plug" size={16} class="text-surface-400" />
		{m.signal_source_settings()}
		<Icon icon="tabler:chevron-down" size={14} class="text-surface-400" />
	</button>

	{#snippet content()}
		<div class="flex flex-col gap-2">
			<h3 class="text-sm font-semibold">{m.signal_source_card_title()}</h3>

			<label class="flex items-center gap-2">
				<input
					type="radio"
					bind:group={sourceKind}
					value="off"
					disabled={sourceSaving}
					onchange={() => applySource('off')}
				/>
				<span>{m.signal_source_off()}</span>
			</label>

			<label class="flex items-center gap-2">
				<input
					type="radio"
					bind:group={sourceKind}
					value="file"
					disabled={sourceSaving}
					onchange={() => applySource('file')}
				/>
				<span>{m.signal_source_file()}</span>
			</label>
			{#if sourceKind === 'file'}
				<div class="pl-6">
					<Input
						label={m.signal_source_file_path_label()}
						placeholder={m.signal_source_file_path_placeholder()}
						bind:value={filePath}
						onValueChange={handleFilePathCommit}
					/>
				</div>
			{/if}

			<label class="flex items-center gap-2">
				<input
					type="radio"
					bind:group={sourceKind}
					value="server"
					disabled={sourceSaving}
					onchange={() => applySource('server')}
				/>
				<span>{m.signal_source_server()}</span>
			</label>
			{#if sourceKind === 'server'}
				<div class="pl-6">
					{#if serverOptions.length === 0}
						<p class="text-surface-400 text-sm">{m.signal_source_no_servers()}</p>
					{:else}
						<Select
							options={serverOptions}
							bind:value={selectedServerId}
							onChange={handleServerChange}
							placeholder={m.signal_source_server_placeholder()}
						/>
					{/if}
				</div>
			{/if}

			{#if status?.source.error}
				<p class="text-error-400 text-sm">
					{m.signal_source_error({ message: status.source.error })}
				</p>
			{/if}
			{#if sourceError}
				<p class="text-error-400 text-sm">{sourceError}</p>
			{/if}
		</div>
	{/snippet}
</Popover>
