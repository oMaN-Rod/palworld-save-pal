<script lang="ts">
	import { untrack } from 'svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import type { GameInstanceFields, GameTestInstanceJson } from '$states/gameState.svelte';

	let {
		initial,
		editing = false,
		ontest,
		onsave,
		oncancel
	}: {
		initial: GameInstanceFields;
		editing?: boolean;
		ontest: (fields: GameInstanceFields) => Promise<GameTestInstanceJson>;
		onsave: (fields: GameInstanceFields) => void | Promise<void>;
		oncancel: () => void;
	} = $props();

	let name = $state(untrack(() => initial.name));
	let host = $state(untrack(() => initial.host));
	let port = $state(untrack(() => initial.port));
	let token = $state(untrack(() => initial.token));
	let testResult = $state<GameTestInstanceJson | null>(null);
	let testing = $state(false);
	let saving = $state(false);

	const fields = $derived<GameInstanceFields>({
		name: name.trim(),
		host: host.trim(),
		port,
		token
	});
	const valid = $derived(
		fields.host.length > 0 && fields.token.length > 0 && port >= 1 && port <= 65535
	);
	const isRemote = $derived(fields.host !== '127.0.0.1' && fields.host !== 'localhost');

	async function test() {
		if (testing) return;
		testing = true;
		try {
			testResult = await ontest(fields);
		} finally {
			testing = false;
		}
	}

	async function save() {
		if (!valid || saving) return;
		saving = true;
		try {
			await onsave(fields);
		} finally {
			saving = false;
		}
	}
</script>

<div class="flex flex-col gap-3">
	<label class="flex flex-col gap-1 text-xs">
		<span class="text-surface-300">{m.live_instance_name()}</span>
		<input class="bg-surface-800 rounded-sm px-2 py-1" bind:value={name} />
	</label>
	<label class="flex flex-col gap-1 text-xs">
		<span class="text-surface-300">{m.live_instance_host()}</span>
		<input class="bg-surface-800 rounded-sm px-2 py-1" bind:value={host} />
	</label>
	<label class="flex flex-col gap-1 text-xs">
		<span class="text-surface-300">{m.live_instance_port()}</span>
		<input
			type="number"
			min="1"
			max="65535"
			class="bg-surface-800 rounded-sm px-2 py-1"
			bind:value={port}
		/>
	</label>
	<label class="flex flex-col gap-1 text-xs">
		<span class="text-surface-300">{m.live_instance_token()}</span>
		<input type="password" class="bg-surface-800 rounded-sm px-2 py-1" bind:value={token} />
		{#if editing}
			<span class="text-surface-500">{m.live_instance_token_hint()}</span>
		{/if}
	</label>

	{#if isRemote}
		<p class="text-xs text-yellow-400">{m.live_instance_remote_warning()}</p>
	{/if}

	{#if testResult}
		<p aria-live="polite" class={testResult.ok ? 'text-xs text-green-400' : 'text-xs text-red-400'}>
			{testResult.ok ? m.live_instance_test_ok() : m.live_instance_test_failed()}
		</p>
	{/if}

	<div class="flex justify-end gap-2">
		<button
			type="button"
			data-action="test"
			disabled={testing}
			class="rounded-sm px-2 py-1 text-xs disabled:opacity-50"
			onclick={test}
		>
			{m.live_instance_test()}
		</button>
		<button
			type="button"
			data-action="cancel"
			class="rounded-sm px-2 py-1 text-xs"
			onclick={oncancel}
		>
			{m.cancel()}
		</button>
		<button
			type="button"
			data-action="save"
			disabled={!valid || saving}
			class="bg-primary-600 rounded-sm px-2 py-1 text-xs disabled:opacity-50"
			onclick={save}
		>
			{c.save}
		</button>
	</div>
</div>
