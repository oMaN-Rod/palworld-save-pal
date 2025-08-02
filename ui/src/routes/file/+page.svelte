<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { goto } from '$app/navigation';
	import { getAppState, getModalState } from '$states';
	import { Card } from '$components/ui';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import { GamepassSaveList, TextInputModal } from '$components';

	const appState = getAppState();
	const modal = getModalState();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);

	const totalPals = $derived.by(() => {
		return Object.values(appState.players).reduce(
			(total: number, player) => total + (player.pals ? Object.values(player.pals).length : 0),
			0
		);
	});

	const totalBases = $derived.by(() => {
		return Object.values(appState.guilds).reduce(
			(total: number, guild) => total + (guild.bases ? Object.values(guild.bases).length : 0),
			0
		);
	});

	async function handleSelectSave(saveType: string) {
		await goto('/loading');
		send(MessageType.SELECT_SAVE, {
			type: saveType,
			local: isDesktopMode
		});
	}

	$effect(() => {
		if (!isDesktopMode) goto('/upload');
	});

	const saveOptions = [
		{
			type: 'steam',
			title: 'Steam',
			icon: steamIcon,
			description: 'Steam/Mac: Level.sav save file',
			disabled: false
		},
		{
			type: 'gamepass',
			title: 'Xbox Game Pass',
			icon: xboxIcon,
			description: 'Xbox Game Pass: container.index file',
			disabled: false
		}
	];

	async function handleEditWorldName() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit World Name',
			value: appState.saveFile!.world_name
		});
		if (!result) return;
		appState.saveFile!.world_name = result;
		send(MessageType.RENAME_WORLD, result);
	}
</script>

<div class="bg-surface-900 flex min-h-screen w-full flex-col items-center justify-center py-12">
	<div class="flex w-full max-w-3xl flex-col gap-12">
		<section class="w-full">
			<h1 class="text-primary-400 mb-6 text-center text-4xl font-extrabold tracking-tight">
				Select Save Platform
			</h1>
			<div class="grid w-full grid-cols-1 justify-center gap-8 sm:grid-cols-2">
				{#each saveOptions as option}
					<button
						type="button"
						class={cn(
							'bg-surface-800 flex flex-col items-center justify-between rounded-xl border-2 p-8 shadow-md transition-all',
							option.disabled
								? 'border-surface-700 cursor-not-allowed opacity-50'
								: 'hover:border-primary-500 border-surface-700 cursor-pointer'
						)}
						onclick={() => !option.disabled && handleSelectSave(option.type)}
						disabled={option.disabled}
					>
						<div class="flex flex-col items-center gap-2">
							<div
								class="bg-surface-900 mb-2 flex h-24 w-24 items-center justify-center rounded-full p-4 shadow"
							>
								{@html option.icon}
							</div>
							<span class="text-xl font-semibold text-white">{option.title}</span>
							<p class="text-surface-300 text-center text-base">{option.description}</p>
						</div>
					</button>
				{/each}
			</div>
		</section>

		{#if appState.saveFile && appState.playerSaveFiles}
			<div class="flex flex-col space-y-2">
				<div class="flex justify-center">
					<button class="btn hover:text-secondary-500 h4" onclick={handleEditWorldName}>
						{appState.saveFile.world_name}
					</button>
				</div>
				<Card class="min-w-96">
					<div class="grid grid-cols-[auto_1fr] gap-2">
						<span class="font-bold">Players:</span>
						<span class="text-end">{appState.playerSaveFiles.length}</span>
						<span class="font-bold">Pals:</span>
						<span class="text-end">{totalPals}</span>
						<span class="font-bold">Guilds:</span>
						<span class="text-end">{Object.values(appState.guilds).length}</span>
						<span class="font-bold">Bases:</span>
						<span class="text-end">{totalBases}</span>
					</div>
				</Card>
			</div>
		{:else if appState.gamepassSaves && Object.keys(appState.gamepassSaves).length > 0}
			<GamepassSaveList bind:saves={appState.gamepassSaves} />
		{/if}
	</div>
</div>
