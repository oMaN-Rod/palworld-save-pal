<script lang="ts">
	import type { Server } from '$types';
	import { Button, Input } from '$components/ui';
	import { getServerState } from '$states';
	import { Save, ExternalLink } from 'lucide-svelte';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { envGroups, PALWORLD_DOCS_URL } from './envGroups';
	import EnvField from './EnvField.svelte';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();

	// Local editable state
	let serverName = $state(server.server_name);
	let serverDescription = $state(server.server_description);
	let serverPassword = $state(server.server_password);
	let adminPassword = $state(server.admin_password);
	let maxPlayers = $state(server.max_players);
	let envVars = $state<Record<string, string>>({ ...server.env_vars });

	// Sync local state when server changes
	$effect(() => {
		serverName = server.server_name;
		serverDescription = server.server_description;
		serverPassword = server.server_password;
		adminPassword = server.admin_password;
		maxPlayers = server.max_players;
		envVars = { ...server.env_vars };
	});

	function getEnvValue(key: string, defaultValue: string): string {
		return envVars[key] ?? defaultValue;
	}

	function setEnvValue(key: string, value: string) {
		envVars[key] = value;
		envVars = envVars;
	}

	const saving = $derived(serverState.saving);

	async function handleSave() {
		await serverState.updateServer(server.id, {
			server_name: serverName,
			server_description: serverDescription,
			server_password: serverPassword,
			admin_password: adminPassword,
			max_players: maxPlayers,
			env_vars: envVars
		});
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-3">
			<h3 class="text-lg font-bold">Server Settings</h3>
			<a
				href={PALWORLD_DOCS_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="text-surface-400 hover:text-primary-400 flex items-center gap-1 text-xs transition-colors"
			>
				<ExternalLink size={12} />
				Official Docs
			</a>
		</div>
		<Button variant="primary" size="sm" onclick={handleSave} disabled={saving}>
			<Save size={14} />
			{saving ? 'Saving...' : 'Save Changes'}
		</Button>
	</div>

	<!-- Core Settings -->
	<div class="grid grid-cols-2 gap-3">
		<Input label="Server Name" bind:value={serverName} />
		<Input label="Server Description" bind:value={serverDescription} />
		<Input label="Server Password" bind:value={serverPassword} placeholder="(optional)" />
		<Input label="Admin Password" bind:value={adminPassword} />
		<Input label="Max Players" type="number" bind:value={maxPlayers} min={1} max={32} />
	</div>

	<!-- ENV Variable Groups -->
	<Accordion collapsible>
		{#each envGroups as group (group.title)}
			<Accordion.Item
				value={group.title}
				base="rounded-sm bg-surface-900"
				controlHover="hover:bg-secondary-500/25"
			>
				{#snippet control()}
					<span class="text-sm font-medium">{group.title}</span>
				{/snippet}
				{#snippet panel()}
					<div class="grid grid-cols-2 gap-2 p-3">
						{#each group.keys as ek (ek.key)}
							<EnvField
								envKey={ek}
								value={getEnvValue(ek.key, ek.default)}
								onchange={setEnvValue}
							/>
						{/each}
					</div>
				{/snippet}
			</Accordion.Item>
		{/each}
	</Accordion>
</div>
