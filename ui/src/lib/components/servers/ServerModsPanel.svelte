<script lang="ts">
	import type { Server, ServerMod } from '$types';
	import { getServerState } from '$states';
	import { Card } from '$components/ui';
	import { Upload, Package, Shield } from 'lucide-svelte';
	import { cn } from '$theme';

	let { server } = $props<{ server: Server }>();

	const serverState = getServerState();

	const mods = $derived(serverState.mods);

	// Group mods by type for display
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

	type ModInstallType = 'ue4ss' | 'logic' | 'native';
	let installType: ModInstallType = $state('ue4ss');

	async function handleInstall(event: Event) {
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

	const modTypeLabel: Record<string, string> = {
		ue4ss: 'UE4SS Lua',
		logic: 'Logic Mod',
		native: 'Native DLL'
	};

	const modTypeColor: Record<string, string> = {
		ue4ss: 'text-blue-400',
		logic: 'text-purple-400',
		native: 'text-orange-400'
	};
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between">
		<h3 class="text-lg font-bold">Mods</h3>
		<div class="flex items-center gap-2">
			<!-- Mod type selector for install -->
			<div class="flex rounded-sm overflow-hidden text-xs">
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
				<input type="file" accept=".zip" class="hidden" onchange={handleInstall} />
			</label>
		</div>
	</div>

	{#if mods.length === 0}
		<Card class="text-surface-400 text-center">
			<Package size={32} class="mx-auto mb-2 opacity-50" />
			<p>No mods installed</p>
		</Card>
	{:else}
		<!-- Native DLL mods -->
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

		<!-- UE4SS mods -->
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

		<!-- Logic mods -->
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
</div>
