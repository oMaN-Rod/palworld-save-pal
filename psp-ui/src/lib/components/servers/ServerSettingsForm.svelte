<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Server } from '$types';
	import { Button, Input } from '$components/ui';
	import { getServerState } from '$states';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { envGroups, PALWORLD_DOCS_URL } from './envGroups';
	import EnvField from './EnvField.svelte';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();

	let name = $state(server.name);
	let serverName = $state(server.server_name);
	let serverDescription = $state(server.server_description);
	let serverPassword = $state(server.server_password);
	let adminPassword = $state(server.admin_password);
	let maxPlayers = $state(server.max_players);
	let gamePort = $state(server.game_port);
	let queryPort = $state(server.query_port);
	let restApiPort = $state(server.rest_api_port);
	let launchArgs = $state(server.launch_args ?? '');
	let workshopDir = $state(server.workshop_dir ?? '');
	let steamcmdPath = $state(server.steamcmd_path ?? '');
	let envVars = $state<Record<string, string>>({ ...server.env_vars });

	let detectingWorkshop = $state(false);

	const isNative = $derived(server.server_type === 'native');

	$effect(() => {
		name = server.name;
		serverName = server.server_name;
		serverDescription = server.server_description;
		serverPassword = server.server_password;
		adminPassword = server.admin_password;
		maxPlayers = server.max_players;
		gamePort = server.game_port;
		queryPort = server.query_port;
		restApiPort = server.rest_api_port;
		launchArgs = server.launch_args ?? '';
		workshopDir = server.workshop_dir ?? '';
		steamcmdPath = server.steamcmd_path ?? '';
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

	async function handleDetectWorkshop() {
		detectingWorkshop = true;
		serverState.detectedWorkshopDir = '';
		await serverState.detectWorkshopDir();
		const start = Date.now();
		const check = () => {
			if (serverState.detectedWorkshopDir || Date.now() - start > 5000) {
				if (serverState.detectedWorkshopDir) {
					workshopDir = serverState.detectedWorkshopDir;
				}
				detectingWorkshop = false;
				return;
			}
			setTimeout(check, 200);
		};
		check();
	}

	async function handleSave() {
		const updates: Record<string, any> = {
			name,
			server_name: serverName,
			server_description: serverDescription,
			server_password: serverPassword,
			admin_password: adminPassword,
			max_players: maxPlayers,
			game_port: gamePort,
			query_port: queryPort,
			rest_api_port: restApiPort,
			env_vars: envVars
		};

		if (isNative) {
			updates.launch_args = launchArgs;
			updates.workshop_dir = workshopDir;
			updates.steamcmd_path = steamcmdPath;
		}

		await serverState.updateServer(server.id, updates);
	}
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between">
		<div class="flex flex-col">
			<div class="flex items-center gap-3">
				<h3 class="text-lg font-bold">Server Settings</h3>
				<a
					href={PALWORLD_DOCS_URL}
					target="_blank"
					rel="noopener noreferrer"
					class="text-surface-400 hover:text-primary-400 flex items-center gap-1 text-xs transition-colors"
				>
					<Icon icon="tabler:external-link" size={12} />
					Official Docs
				</a>
			</div>
			<div>
				<p class="text-surface-400 text-xs">Install Path</p>
				<p class="text-surface-200 font-mono text-xs break-all">{server.install_path}</p>
			</div>
		</div>
		<Button variant="primary" size="sm" onclick={handleSave} disabled={saving}>
			<Icon icon="tabler:device-floppy" size={14} />
			{saving ? 'Saving...' : 'Save Changes'}
		</Button>
	</div>

	<p class="text-surface-400 text-xs">
		Saving {isNative
			? 'rewrites PalWorldSettings.ini and restarts the server if it is running.'
			: 'recreates the container, which restarts the server if it is running.'}
	</p>

	<div class="grid grid-cols-2 gap-3">
		<Input label="Display Name" bind:value={name} />
		<Input label="Server Name (in-game)" bind:value={serverName} />
		<Input label="Server Description" bind:value={serverDescription} />
		<Input label="Server Password" bind:value={serverPassword} placeholder="(optional)" />
		<Input label="Admin Password" bind:value={adminPassword} />
		<Input label="Max Players" type="number" bind:value={maxPlayers} min={1} max={32} />
	</div>

	<div class="grid grid-cols-3 gap-3">
		<Input label="Game Port" type="number" bind:value={gamePort} />
		<Input label="Query Port" type="number" bind:value={queryPort} />
		<Input label="REST API Port" type="number" bind:value={restApiPort} />
	</div>

	{#if isNative}
		<div class="flex flex-col gap-3">
			<Input
				label="Extra Launch Args"
				bind:value={launchArgs}
				placeholder="-publiclobby -NumberOfWorkerThreadsServer=8"
			/>
			<div class="flex items-center gap-2">
				<div class="flex-1">
					<Input
						label="Steam Workshop Dir"
						bind:value={workshopDir}
						placeholder="Browse or detect to set"
					/>
				</div>
				<Button
					type="button"
					variant="neutral"
					size="sm"
					class="mb-0.5"
					onclick={handleDetectWorkshop}
					disabled={detectingWorkshop}
				>
					<Icon icon="tabler:search" size={14} />
					{detectingWorkshop ? 'Detecting...' : 'Detect'}
				</Button>
			</div>
			<Input
				label="SteamCMD Path"
				bind:value={steamcmdPath}
				placeholder="Leave empty to auto-detect or download"
			/>
		</div>
	{/if}

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
