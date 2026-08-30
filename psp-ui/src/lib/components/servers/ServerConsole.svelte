<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Server } from '$types';
	import { getServerState } from '$states';
	import { Button, Card, Input } from '$components/ui';
	import { cn } from '$theme';
	import { JSONEditor } from 'svelte-jsoneditor';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();
	const apiResponse = $derived(serverState.apiResponse);

	let editorContent = $derived.by(() => {
		if (apiResponse && apiResponse.server_id === server.id) {
			return { json: apiResponse.result.data };
		}
		return { json: {} };
	});

	type ApiEndpoint = {
		id: string;
		label: string;
		method: string;
		hasPayload: boolean;
		payloadTemplate?: Record<string, string>;
		/** Lowercase launch argument the game server must be started with. */
		requiresLaunchArg?: string;
	};

	const endpoints: ApiEndpoint[] = [
		{ id: 'info', label: 'Server Info', method: 'GET', hasPayload: false },
		{ id: 'players', label: 'Players', method: 'GET', hasPayload: false },
		{ id: 'settings', label: 'Settings', method: 'GET', hasPayload: false },
		{ id: 'metrics', label: 'Metrics', method: 'GET', hasPayload: false },
		{
			id: 'game-data',
			label: 'Game Data',
			method: 'GET',
			hasPayload: false,
			requiresLaunchArg: '-enable-gamedata-api'
		},
		{ id: 'save', label: 'Save World', method: 'POST', hasPayload: false },
		{
			id: 'shutdown',
			label: 'Shutdown',
			method: 'POST',
			hasPayload: true,
			payloadTemplate: { waittime: '10', message: 'Server shutting down...' }
		},
		{ id: 'stop', label: 'Force Stop', method: 'POST', hasPayload: false },
		{
			id: 'announce',
			label: 'Announce',
			method: 'POST',
			hasPayload: true,
			payloadTemplate: { message: '' }
		},
		{
			id: 'kick',
			label: 'Kick Player',
			method: 'POST',
			hasPayload: true,
			payloadTemplate: { userid: '', message: 'Kicked' }
		},
		{
			id: 'ban',
			label: 'Ban Player',
			method: 'POST',
			hasPayload: true,
			payloadTemplate: { userid: '', message: 'Banned' }
		},
		{
			id: 'unban',
			label: 'Unban Player',
			method: 'POST',
			hasPayload: true,
			payloadTemplate: { userid: '' }
		}
	];

	let selectedEndpoint = $state<ApiEndpoint>(endpoints[0]);
	let payloadValues = $state<Record<string, string>>({});

	$effect(() => {
		if (selectedEndpoint.payloadTemplate) {
			payloadValues = { ...selectedEndpoint.payloadTemplate };
		} else {
			payloadValues = {};
		}
	});

	const isRunning = $derived(server.status?.running ?? false);
	const hasResponse = $derived(apiResponse && apiResponse.server_id === server.id);

	const launchArgs = $derived(
		(server.launch_args ?? '').toLowerCase().split(/\s+/).filter(Boolean)
	);

	function isAvailable(endpoint: ApiEndpoint): boolean {
		return !endpoint.requiresLaunchArg || launchArgs.includes(endpoint.requiresLaunchArg);
	}

	const missingArgs = $derived([
		...new Set(
			endpoints.filter((ep) => !isAvailable(ep)).map((ep) => ep.requiresLaunchArg as string)
		)
	]);

	const canSend = $derived(isAvailable(selectedEndpoint));

	async function handleCall() {
		const payload = selectedEndpoint.hasPayload ? payloadValues : undefined;
		await serverState.callApi(server.id, selectedEndpoint.id, selectedEndpoint.method, payload);
	}
</script>

<div class="flex flex-col gap-4">
	<h3 class="text-lg font-bold">REST API Console</h3>

	{#if !isRunning}
		<Card class="text-surface-400 text-center">
			<Icon icon="tabler:terminal-2" size={32} class="mx-auto mb-2 opacity-50" />
			<p>Server must be running to use the REST API</p>
		</Card>
	{:else}
		<div class="flex flex-wrap gap-2">
			{#each endpoints as ep (ep.id)}
				{@const available = isAvailable(ep)}
				<button
					class={cn(
						'rounded-sm px-3 py-1.5 text-xs font-medium transition-colors',
						!available
							? 'bg-surface-800 text-surface-500 cursor-not-allowed'
							: selectedEndpoint.id === ep.id
								? 'bg-secondary-500 text-white'
								: 'bg-surface-700 text-surface-300 hover:bg-surface-600'
					)}
					disabled={!available}
					title={available ? undefined : `Requires the ${ep.requiresLaunchArg} launch argument`}
					onclick={() => (selectedEndpoint = ep)}
				>
					<span class="text-surface-400 mr-1 text-[10px]">{ep.method}</span>
					{ep.label}
				</button>
			{/each}
		</div>

		{#each missingArgs as arg (arg)}
			<p class="text-surface-400 flex items-center gap-1.5 text-xs">
				<Icon icon="tabler:info-circle" size={14} class="shrink-0" />
				<span>
					Dimmed endpoints require the server to be launched with
					<code class="bg-surface-800 rounded-sm px-1 py-0.5">{arg}</code>.
				</span>
			</p>
		{/each}

		{#if selectedEndpoint.hasPayload && selectedEndpoint.payloadTemplate}
			<Card padding="p-3">
				<div class="grid grid-cols-2 gap-2">
					{#each Object.keys(selectedEndpoint.payloadTemplate) as key (key)}
						<Input
							label={key}
							value={payloadValues[key] ?? ''}
							onValueChange={(v) => {
								payloadValues[key] = String(v);
								payloadValues = payloadValues;
							}}
						/>
					{/each}
				</div>
			</Card>
		{/if}

		<div class="flex items-center gap-3">
			<Button variant="primary" disabled={!canSend} onclick={handleCall}>
				<Icon icon="tabler:send" size={14} />
				Send Request
			</Button>
			{#if hasResponse}
				<span
					class={cn(
						'rounded-sm px-2 py-0.5 text-xs',
						apiResponse!.result.status_code >= 200 && apiResponse!.result.status_code < 300
							? 'bg-green-500/20 text-green-400'
							: 'bg-red-500/20 text-red-400'
					)}
				>
					{apiResponse!.result.status_code}
				</span>
			{/if}
		</div>

		{#if hasResponse}
			<div class="editor-wrapper max-h-[500px] overflow-auto">
				<JSONEditor content={editorContent} readOnly={true} />
			</div>
		{/if}
	{/if}
</div>

<style>
	.editor-wrapper {
		--jse-theme-color: var(--color-surface-700);
		--jse-theme-color-highlight: var(--color-secondary-500);
		--jse-background-color: var(--color-surface-900);
		--jse-text-color: var(--color-surface-100);
		--jse-panel-background: var(--color-surface-800);
		--jse-panel-border: var(--color-surface-700);
	}
</style>
