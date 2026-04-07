<script lang="ts">
	import type { Server, ServerMod } from '$types';
	import { getServerState } from '$states';
	import { Card } from '$components/ui';
	import { Upload, Package, Shield, FolderOpen } from 'lucide-svelte';
	import { cn } from '$theme';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();

	const mods = $derived(serverState.mods);
	const isNative = $derived(server.server_type === 'native');

	// Docker: Group mods by type for display
	const ue4ssMods = $derived(mods.filter((m) => m.mod_type === 'ue4ss'));
	const logicMods = $derived(mods.filter((m) => m.mod_type === 'logic'));
	const nativeMods = $derived(mods.filter((m) => m.mod_type === 'native'));

	$effect(() => {
		if (server.id) {
			serverState.loadMods(server.id);
		}
	});

	function handleToggle(mod: ServerMod) {
		serverState.toggleMod(server.id, mod.mod_name, !mod.enabled);
	}

	// Docker install controls
	type ModInstallType = 'ue4ss' | 'logic' | 'native';
	let installType: ModInstallType = $state('ue4ss');

	async function handleDockerInstall(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		const modName = file.name.replace(/\.zip$/, '');
		const reader = new FileReader();

		reader.onload = () => {
			const base64 = (reader.result as string).split(',')[1];
			serverState.installMod(server.id, modName, base64, installType);
		};
		reader.readAsDataURL(file);
		input.value = '';
	}

	// Native install (all mods go to Mods/Workshop/)
	async function handleNativeInstall(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		const modName = file.name.replace(/\.zip$/, '');
		const reader = new FileReader();

		reader.onload = () => {
			const base64 = (reader.result as string).split(',')[1];
			serverState.installMod(server.id, modName, base64, 'workshop');
		};
		reader.readAsDataURL(file);
		input.value = '';
	}

	const modTypeLabel: Record<string, string> = {
		ue4ss: 'UE4SS',
		logic: 'Logic Mod',
		native: 'Native DLL',
		lua: 'Lua',
		palschema: 'PalSchema',
		paks: 'Pak',
		unknown: 'Unknown'
	};

	const modTypeColor: Record<string, string> = {
		ue4ss: 'text-blue-400',
		logic: 'text-purple-400',
		native: 'text-orange-400',
		lua: 'text-cyan-400',
		palschema: 'text-green-400',
		paks: 'text-amber-400',
		unknown: 'text-surface-400'
	};

	const sourceLabel: Record<string, string> = {
		workshop: 'Workshop',
		local: 'Local',
		config: 'Config'
	};
</script>

<div class="flex flex-col gap-4">
	{#if isNative}
		<!-- Native server mod panel -->
		<div class="flex items-center justify-between">
			<h3 class="text-lg font-bold">Mods</h3>
			<label
				class="btn bg-primary-500 hover:bg-primary-600 flex cursor-pointer items-center gap-2 rounded-sm px-3 py-1.5 text-sm"
			>
				<Upload size={14} />
				Install (.zip)
				<input type="file" accept=".zip" class="hidden" onchange={handleNativeInstall} />
			</label>
		</div>

		{#if server.workshop_dir}
			<div class="text-surface-400 flex items-center gap-2 text-xs">
				<FolderOpen size={12} />
				Workshop: <span class="text-surface-200 font-mono">{server.workshop_dir}</span>
			</div>
		{:else}
			<div class="text-surface-500 text-xs">
				No Steam Workshop directory configured. Mods can be installed via ZIP upload.
			</div>
		{/if}

		{#if mods.length === 0}
			<Card class="text-surface-400 text-center">
				<Package size={32} class="mx-auto mb-2 opacity-50" />
				<p>No mods found</p>
				<p class="mt-1 text-xs">
					{#if server.workshop_dir}
						Subscribe to mods in Steam Workshop or install via ZIP
					{:else}
						Install mod ZIPs or configure a Steam Workshop directory
					{/if}
				</p>
			</Card>
		{:else}
			<div class="flex flex-col gap-2">
				{#each mods as mod (mod.mod_name)}
					<Card padding="p-3">
						<div class="flex items-center justify-between">
							<div class="flex items-center gap-3">
								<Package
									size={16}
									class={mod.enabled ? modTypeColor[mod.mod_type] || 'text-surface-400' : 'text-surface-500'}
								/>
								<div class="flex flex-col">
									<div class="flex items-center gap-2">
										<span class="text-sm font-medium">
											{mod.display_name || mod.mod_name}
										</span>
										{#if mod.display_name && mod.display_name !== mod.mod_name}
											<span class="text-surface-500 font-mono text-xs">
												{mod.mod_name}
											</span>
										{/if}
									</div>
									<div class="flex items-center gap-2 text-xs">
										<span class={modTypeColor[mod.mod_type] || 'text-surface-400'}>
											{modTypeLabel[mod.mod_type] || mod.mod_type}
										</span>
										{#if mod.mod_version}
											<span class="text-surface-500">v{mod.mod_version}</span>
										{/if}
										{#if mod.mod_author}
											<span class="text-surface-500">by {mod.mod_author}</span>
										{/if}
										{#if mod.source}
											<span
												class={cn(
													'rounded-xs px-1',
													mod.source === 'workshop'
														? 'bg-secondary-500/20 text-secondary-400'
														: 'bg-surface-700 text-surface-400'
												)}
											>
												{sourceLabel[mod.source] || mod.source}
											</span>
										{/if}
									</div>
								</div>
							</div>
							<button
								class="btn btn-sm rounded-sm px-3 py-1 text-xs {mod.enabled
									? 'bg-success-500'
									: 'bg-surface-700 text-surface-400'}"
								onclick={() => handleToggle(mod)}
							>
								{mod.enabled ? 'Enabled' : 'Disabled'}
							</button>
						</div>
					</Card>
				{/each}
			</div>
		{/if}
	{:else}
		<!-- Docker server mod panel (unchanged) -->
		<div class="flex items-center justify-between">
			<h3 class="text-lg font-bold">Mods</h3>
			<div class="flex items-center gap-2">
				<div class="flex overflow-hidden rounded-sm text-xs">
					{#each (['ue4ss', 'logic', 'native'] as const) as t (t)}
						<button
							class={cn(
								'px-2 py-1',
								installType === t
									? 'bg-secondary-500 text-white'
									: 'bg-surface-700 text-surface-400 hover:bg-surface-600'
							)}
							onclick={() => (installType = t)}
						>
							{modTypeLabel[t]}
						</button>
					{/each}
				</div>
				<label
					class="btn bg-primary-500 hover:bg-primary-600 flex cursor-pointer items-center gap-2 rounded-sm px-3 py-1.5 text-sm"
				>
					<Upload size={14} />
					Install
					<input type="file" accept=".zip" class="hidden" onchange={handleDockerInstall} />
				</label>
			</div>
		</div>

		{#if mods.length === 0}
			<Card class="text-surface-400 text-center">
				<Package size={32} class="mx-auto mb-2 opacity-50" />
				<p>No mods installed</p>
			</Card>
		{:else}
			{#if nativeMods.length > 0}
				<div>
					<h4 class="text-surface-400 mb-2 flex items-center gap-2 text-xs font-medium uppercase">
						<Shield size={12} />
						Native / Proxy DLL Mods
					</h4>
					<div class="flex flex-col gap-2">
						{#each nativeMods as mod (mod.mod_name)}
							<Card padding="p-3">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-3">
										<Shield size={16} class="text-orange-400" />
										<div>
											<span class="text-sm font-medium">{mod.mod_name}</span>
											<span class="text-orange-400 ml-2 text-xs">native</span>
										</div>
									</div>
									<span class="text-xs text-green-400">Synced on boot</span>
								</div>
							</Card>
						{/each}
					</div>
				</div>
			{/if}

			{#if ue4ssMods.length > 0}
				<div>
					<h4 class="text-surface-400 mb-2 flex items-center gap-2 text-xs font-medium uppercase">
						<Package size={12} />
						UE4SS Mods
					</h4>
					<div class="flex flex-col gap-2">
						{#each ue4ssMods as mod (mod.mod_name)}
							<Card padding="p-3">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-3">
										<Package
											size={16}
											class={mod.enabled ? 'text-blue-400' : 'text-surface-400'}
										/>
										<span class="text-sm font-medium">{mod.mod_name}</span>
									</div>
									<button
										class="btn btn-sm rounded-sm px-3 py-1 text-xs {mod.enabled
											? 'bg-success-500 '
											: 'bg-surface-700 text-surface-400'}"
										onclick={() => handleToggle(mod)}
									>
										{mod.enabled ? 'Enabled' : 'Disabled'}
									</button>
								</div>
							</Card>
						{/each}
					</div>
				</div>
			{/if}

			{#if logicMods.length > 0}
				<div>
					<h4 class="text-surface-400 mb-2 flex items-center gap-2 text-xs font-medium uppercase">
						<Package size={12} />
						Logic Mods (.pak)
					</h4>
					<div class="flex flex-col gap-2">
						{#each logicMods as mod (mod.mod_name)}
							<Card padding="p-3">
								<div class="flex items-center justify-between">
									<div class="flex items-center gap-3">
										<Package size={16} class="text-purple-400" />
										<span class="text-sm font-medium">{mod.mod_name}</span>
									</div>
									<span class="text-xs text-green-400">Active</span>
								</div>
							</Card>
						{/each}
					</div>
				</div>
			{/if}
		{/if}
	{/if}
</div>
