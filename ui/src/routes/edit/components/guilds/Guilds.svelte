<script lang="ts">
	import { palsData } from '$lib/data';
	import { getAppState, getSocketState } from '$states';
	import { Tooltip } from '$components/ui';
	import { type Pal, EntryState, MessageType } from '$types';
	import { staticIcons } from '$lib/constants';
	import { Ambulance, X, ReplaceAll } from 'lucide-svelte';
	import Card from '$components/ui/card/Card.svelte';
	import { PalCard } from '$components';
	const appState = getAppState();
	const ws = getSocketState();

	let selectedPals: string[] = $state([]);

	const playerGuild = $derived.by(() => {
		if (appState.selectedPlayer?.guild_id) {
			return appState.guilds[appState.selectedPlayer.guild_id];
		}
	});

	const guildBases = $derived.by(() => {
		if (playerGuild) {
			return playerGuild.bases;
		}
	});

	$inspect(appState.selectedPlayer);
	$inspect(playerGuild);
	$inspect(guildBases);

	function handlePalSelect(pal: Pal, event: MouseEvent) {
		if (!pal || pal.character_id === 'None') return;
		if (event.ctrlKey || event.metaKey) {
			if (selectedPals.includes(pal.instance_id)) {
				selectedPals = selectedPals.filter((id) => id !== pal.instance_id);
			} else {
				selectedPals = [...selectedPals, pal.instance_id];
			}
		}
	}

	async function healSelectedPals() {
		if (!appState.guilds || !appState.selectedPlayer) return;
		if (selectedPals.length === 0) return;

		const message = {
			type: MessageType.HEAL_PALS,
			data: [...selectedPals]
		};
		ws.send(JSON.stringify(message));

		Object.values(appState.guilds).forEach(async (guild) => {
			Object.values(guild.bases).forEach(async (base) => {
				Object.values(base.pals).forEach(async (pal) => {
					if (selectedPals.includes(pal.instance_id)) {
						pal.hp = pal.max_hp;
						pal.sanity = 100;
						// pal.state = EntryState.MODIFIED;
						const palData = palsData.pals[pal.character_key];
						if (palData) {
							pal.stomach = palData.max_full_stomach;
						}
					}
				});
			});
		});

		selectedPals = [];
	}

	function handleSelectAll() {
		if (!guildBases) return;

		selectedPals = [
			...Object.values(guildBases)
				.map((base) => Object.values(base.pals).map((pal) => pal.instance_id))
				.flat()
		];
	}
</script>

{#if appState.selectedPlayer}
	{#if !playerGuild}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">No Guild found</h2>
		</div>
	{:else if !guildBases}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">No Guild Bases found</h2>
		</div>
	{:else}
		<div class="flex w-full items-center justify-center">
			<h2 class="h2">Guild Bases: {Object.keys(guildBases).length}</h2>
		</div>

		<div class="btn-group bg-surface-900 mx-4 mt-2 items-center rounded p-1">
			<Tooltip>
				<button class="btn hover:preset-tonal-secondary p-2" onclick={() => handleSelectAll()}>
					<ReplaceAll />
				</button>
				{#snippet popup()}
					<div class="flex flex-col">
						<span>Select all in</span>
						<div class="grid grid-cols-[auto_1fr] gap-1">
							<img src={staticIcons.leftClickIcon} alt="Left Click" class="h-6 w-6" />
							<span class="text-sm">pal box</span>
						</div>
					</div>
				{/snippet}
			</Tooltip>
			{#if selectedPals.length > 0}
				<Tooltip>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={healSelectedPals}>
						<Ambulance />
					</button>
					{#snippet popup()}
						Heal selected pal(s)
					{/snippet}
				</Tooltip>
				<Tooltip>
					<button class="btn hover:preset-tonal-secondary p-2" onclick={() => (selectedPals = [])}>
						<X />
					</button>
					{#snippet popup()}
						Clear selected
					{/snippet}
				</Tooltip>
			{/if}
		</div>
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
			{#each Object.values(guildBases) as guildBase (guildBase.id)}
				{#if guildBase.pals && Object.keys(guildBase.pals).length > 0}
					<div class="p-4">
						<div class="mt-2 flex">
							<Card rounded="rounded-none" class="w-full p-4">
								<div class="flex flex-col space-y-2">
									<h6 class="h6 mr-4">Base</h6>
									{#each Object.values(guildBase.pals) as pal, palIndex}
										<PalCard
											{pal}
											bind:selected={selectedPals}
											onSelect={handlePalSelect}
											onMove={() => {}}
											onDelete={() => {}}
											onAdd={() => {}}
											onClone={() => {}}
										/>
									{/each}
								</div>
							</Card>
						</div>
					</div>
				{:else}
					<div class="flex w-full items-center justify-center">
						<h2 class="h2">No Pals at base</h2>
					</div>
				{/if}
			{/each}
		</div>
	{/if}
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Select a Player to view Guilds</h2>
	</div>
{/if}
