<script lang="ts">
	import { untrack } from 'svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import type { GameInstanceFields, GameTestInstanceJson } from '$states/gameState.svelte';
	import { Button, Input } from '$components/ui';

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
		if (!valid || testing) return;
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

<div class="flex flex-col gap-3 bg-surface-900 p-8 rounded-md">
	<Input bind:value={name} label={m.live_instance_name()} />
	<Input bind:value={host} label={m.live_instance_host()} />
	<Input bind:value={port} label={m.live_instance_port()} type="number" min={1} max={65535} />
	<Input bind:value={token} label={m.live_instance_token()} type="password" {editing} hint={editing ? m.live_instance_token_hint() : undefined} />

	{#if isRemote}
		<p class="text-xs text-yellow-400">{m.live_instance_remote_warning()}</p>
	{/if}

	{#if testResult}
		<p aria-live="polite" class={testResult.ok ? 'text-xs text-green-400' : 'text-xs text-red-400'}>
			{testResult.ok ? m.live_instance_test_ok() : m.live_instance_test_failed()}
		</p>
	{/if}

	<div class="flex justify-end gap-2">
		<Button variant="ghost" disabled={!valid || testing} onClick={test}>{m.live_instance_test()}</Button>
		<Button variant="ghost" onClick={oncancel}>{m.cancel()}</Button>
		<Button variant="primary" disabled={!valid || saving} onClick={save}>{c.save}</Button>
	</div>
</div>
