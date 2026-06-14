<script lang="ts">
	import type { Server } from '$types';
	import { getServerState } from '$states';
	import { Button, Card, Input } from '$components/ui';
	import { Send, Terminal } from 'lucide-svelte';
	import { cn } from '$theme';
	import { JSONEditor } from 'svelte-jsoneditor';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();
	const apiResponse = $derived(serverState.apiResponse);

	// JSONEditor content - updates when apiResponse changes
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
	};

	const endpoints: ApiEndpoint[] = [
		{ id: 'info', label: 'Server Info', method: 'GET', hasPayload: false },
		{ id: 'players', label: 'Players', method: 'GET', hasPayload: false },
		{ id: 'settings', label: 'Settings', method: 'GET', hasPayload: false },
		{ id: 'metrics', label: 'Metrics', method: 'GET', hasPayload: false },
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

	async function handleCall() {
		const payload = selectedEndpoint.hasPayload ? payloadValues : undefined;
		await serverState.callApi(server.id, selectedEndpoint.id, selectedEndpoint.method, payload);
	}
</script>

<div class="flex flex-col gap-4">
	<h3 class="text-lg font-bold">REST API Console</h3>

	{#if !isRunning}
		<Card class="text-surface-400 text-center">
			<Terminal size={32} class="mx-auto mb-2 opacity-50" />
			<p>Server must be running to use the REST API</p>
		</Card>
	{:else}
		<!-- Endpoint selector -->
		<div class="flex flex-wrap gap-2">
			{#each endpoints as ep (ep.id)}
				<button
					class={cn(
						'rounded-sm px-3 py-1.5 text-xs font-medium transition-colors',
						selectedEndpoint.id === ep.id
							? 'bg-secondary-500 text-white'
							: 'bg-surface-700 text-surface-300 hover:bg-surface-600'
					)}
					onclick={() => (selectedEndpoint = ep)}
				>
					<span class="text-surface-400 mr-1 text-[10px]">{ep.method}</span>
					{ep.label}
				</button>
			{/each}
		</div>

		<!-- Payload inputs -->
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

		<!-- Send button -->
		<div class="flex items-center gap-3">
			<Button variant="primary" onclick={handleCall}>
				<Send size={14} />
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

		<!-- Response viewer -->
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
