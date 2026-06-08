<script lang="ts">
	import { Button, Card, Input, Tooltip } from '$components/ui';
	import { Save, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { cn } from '$theme';
	import { ExternalLink } from 'lucide-svelte';
	import type { CreateServerData, ServerType } from '$types';
	import { getServerState } from '$states';
	import { envGroups, PALWORLD_DOCS_URL } from './envGroups';
	import EnvField from './EnvField.svelte';
	import { Search } from 'lucide-svelte';

	let {
		title = 'Create Server',
		suggestedPorts = { game_port: 8211, query_port: 27015, rest_api_port: 8212 },
		closeModal
	} = $props<{
		title?: string;
		suggestedPorts?: { game_port: number; query_port: number; rest_api_port: number };
		closeModal: (value: any) => void;
	}>();

	let modalContainer: HTMLDivElement;

	// --- Deployment type ---
	let serverType: ServerType = $state('docker');

	// --- Tab state ---
	type Tab = 'general' | 'gameplay' | 'advanced';
	let activeTab: Tab = $state('general');

	const tabs: { id: Tab; label: string }[] = [
		{ id: 'general', label: 'General' },
		{ id: 'gameplay', label: 'Gameplay' },
		{ id: 'advanced', label: 'Advanced' }
	];

	// General tab groups (shown directly)
	const generalGroupTitles = ['Server Settings', 'REST API & Logging'];
	// Gameplay tab groups
	const gameplayGroupTitles = [
		'Gameplay Rates',
		'Time & Difficulty',
		'PvP / Hardcore',
		'Guild / Building',
		'Items & Drops'
	];
	// Advanced tab groups
	const advancedGroupTitles = [
		'Backup Settings',
		'Auto Update / Reboot',
		'Discord Integration',
		'UE4SS / Mods',
		'Engine Settings',
		'Randomizer'
	];

	function groupsForTab(tab: Tab) {
		const titles =
			tab === 'general'
				? generalGroupTitles
				: tab === 'gameplay'
					? gameplayGroupTitles
					: advancedGroupTitles;
		return envGroups.filter((g) => titles.includes(g.title));
	}

	// --- Form state ---
	let name = $state('');
	let containerName = $state('');
	let serverName = $state('PSP Palworld Server');
	let serverDescription = $state('');
	let serverPassword = $state('');
	let adminPassword = $state('admin');
	let maxPlayers = $state(16);
	let gamePort = $state(suggestedPorts.game_port);
	let queryPort = $state(suggestedPorts.query_port);
	let restApiPort = $state(suggestedPorts.rest_api_port);
	// Pre-populate envVars with all defaults so every displayed value is submitted
	let envVars = $state<Record<string, string>>(
		Object.fromEntries(envGroups.flatMap((g) => g.keys.map((k) => [k.key, k.default])))
	);

	const serverState = getServerState();

	// Native-specific fields
	let steamcmdPath = $state('');
	let installBasePath = $state('');
	let worldName = $state('');
	let launchArgs = $state('');
	let workshopDir = $state('');
	let detectingWorkshop = $state(false);

	const autoContainerName = $derived(
		name
			.toLowerCase()
			.replace(/[^a-z0-9-]/g, '-')
			.replace(/-+/g, '-')
			.replace(/^-|-$/g, '')
	);

	const installPath = $derived(
		installBasePath && worldName ? `${installBasePath.replace(/[\\/]+$/, '')}\\${worldName}` : ''
	);

	$effect(() => {
		if (!containerName || containerName === autoContainerName) {
			containerName = autoContainerName;
		}
	});

	function getEnvValue(key: string, defaultValue: string): string {
		return envVars[key] ?? defaultValue;
	}

	function setEnvValue(key: string, value: string) {
		envVars[key] = value;
		envVars = envVars;
	}

	async function handleDetectWorkshop() {
		detectingWorkshop = true;
		serverState.detectedWorkshopDir = '';
		await serverState.detectWorkshopDir();
		// Wait briefly for the WS response
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

	function handleSubmit() {
		if (!name.trim()) return;

		const data: CreateServerData = {
			name: name.trim(),
			container_name: containerName || autoContainerName,
			server_type: serverType,
			game_port: gamePort,
			query_port: queryPort,
			rest_api_port: restApiPort,
			server_name: serverName,
			server_description: serverDescription,
			server_password: serverPassword,
			admin_password: adminPassword,
			max_players: maxPlayers,
			env_vars: envVars
		};

		if (serverType === 'native') {
			data.steamcmd_path = steamcmdPath;
			data.install_path = installPath;
			data.launch_args = launchArgs;
			data.workshop_dir = workshopDir;
		}

		closeModal(data);
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="max-w-[750px] min-w-[650px]">
		<div class="mb-4 flex items-center gap-3">
			<h3 class="h3">{title}</h3>
			<a
				href={PALWORLD_DOCS_URL}
				target="_blank"
				rel="noopener noreferrer"
				class="text-surface-400 hover:text-secondary-400 flex items-center gap-1 text-xs transition-colors"
			>
				<ExternalLink size={12} />
				Official Docs
			</a>
		</div>

		<!-- Deployment Type Selector -->
		<div class="bg-surface-800 mb-4 flex gap-1 rounded-sm p-1">
			<button
				class={cn(
					'flex-1 rounded-sm px-3 py-1.5 text-sm font-medium transition-colors',
					serverType === 'docker'
						? 'bg-secondary-500/20 text-secondary-400'
						: 'text-surface-400 hover:text-surface-200'
				)}
				onclick={() => (serverType = 'docker')}
			>
				Docker
			</button>
			<button
				class={cn(
					'flex-1 rounded-sm px-3 py-1.5 text-sm font-medium transition-colors',
					serverType === 'native'
						? 'bg-secondary-500/20 text-secondary-400'
						: 'text-surface-400 hover:text-surface-200'
				)}
				onclick={() => (serverType = 'native')}
			>
				Native (Windows)
			</button>
		</div>

		<!-- Tabs -->
		<div class="border-surface-700 mb-4 flex gap-1 border-b">
			{#each tabs as tab (tab.id)}
				<button
					class={cn(
						'px-4 py-2 text-sm font-medium transition-colors',
						activeTab === tab.id
							? 'text-secondary-400 border-secondary-400 border-b-2'
							: 'text-surface-400 hover:text-surface-200 border-b-2 border-transparent'
					)}
					onclick={() => (activeTab = tab.id)}
				>
					{tab.label}
				</button>
			{/each}
		</div>

		<!-- Tab Content -->
		<div class="max-h-[60vh] overflow-y-auto pr-1">
			{#if activeTab === 'general'}
				<div class="flex flex-col gap-3">
					<Input label="Display Name" bind:value={name} placeholder="My Palworld Server" />

					{#if serverType === 'docker'}
						<Input
							label="Container Name"
							bind:value={containerName}
							placeholder={autoContainerName}
						/>
					{:else}
						<!-- Native server fields -->
						<Input
							label="SteamCMD Path (optional, auto-detected)"
							bind:value={steamcmdPath}
							placeholder="Leave empty to auto-detect or download"
						/>
						<Input
							label="Install Base Path"
							bind:value={installBasePath}
							placeholder="C:\PalworldServers"
						/>
						<Input label="World Name (folder name)" bind:value={worldName} placeholder="MyWorld" />
						{#if installPath}
							<p class="text-surface-400 -mt-1 text-xs">
								Install path: <span class="text-surface-200 font-mono">{installPath}</span>
							</p>
						{/if}
						<Input
							label="Extra Launch Args"
							bind:value={launchArgs}
							placeholder="-publiclobby -NumberOfWorkerThreadsServer=8"
						/>
						<div class="flex items-end gap-2">
							<div class="flex-1">
								<Input
									label="Steam Workshop Dir (optional)"
									bind:value={workshopDir}
									placeholder="Auto-detected on create, or browse to set"
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
								<Search size={14} />
								{detectingWorkshop ? 'Detecting...' : 'Detect'}
							</Button>
						</div>
					{/if}

					<Input label="Server Name (in-game)" bind:value={serverName} />
					<Input label="Server Description" bind:value={serverDescription} />

					<div class="grid grid-cols-3 gap-3">
						<Input label="Game Port" type="number" bind:value={gamePort} />
						<Input label="Query Port" type="number" bind:value={queryPort} />
						<Input label="REST API Port" type="number" bind:value={restApiPort} />
					</div>

					<div class="grid grid-cols-2 gap-3">
						<Input label="Server Password" bind:value={serverPassword} placeholder="(optional)" />
						<Input label="Admin Password" bind:value={adminPassword} />
					</div>

					<Input label="Max Players" type="number" bind:value={maxPlayers} min={1} max={32} />

					<!-- General ENV groups -->
					<Accordion collapsible>
						{#each groupsForTab('general') as group (group.title)}
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
			{:else}
				<!-- Gameplay or Advanced tab - accordion groups -->
				<Accordion collapsible>
					{#each groupsForTab(activeTab) as group (group.title)}
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
			{/if}
		</div>

		<div class="mt-4 flex justify-end gap-2">
			<Tooltip position="bottom">
				{#snippet children()}
					<Button
						variant="ghost"
						size="icon"
						onclick={handleSubmit}
						disabled={!name.trim() || (serverType === 'native' && !installPath)}
						data-modal-primary
					>
						<Save />
					</Button>
				{/snippet}
				{#snippet popup()}
					<span>Create</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				{#snippet children()}
					<Button variant="ghost" size="icon" onclick={() => closeModal(null)}>
						<X />
					</Button>
				{/snippet}
				{#snippet popup()}
					<span>Cancel</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
