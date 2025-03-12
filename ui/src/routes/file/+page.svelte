<script lang="ts">
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { goto } from '$app/navigation';
	import { getAppState } from '$states';
	import { Card, Tooltip } from '$components/ui';
	import { send } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { cn } from '$theme';
	import { GamepassSaveList } from '$components';

	type SaveType = 'steam' | 'gamepass' | 'sftp';

	const appState = getAppState();

	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	const steamIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/steam.svg`);
	const xboxIcon = assetLoader.loadSvg(`${ASSET_DATA_PATH}/img/app/xbox.svg`);
	const morpheus = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/app/morpheus.png`);

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

	function splitPath(path: string) {
		if (!path) {
			return {
				directory: '',
				filename: ''
			};
		}
		const normalizedPath = path.replace(/\\/g, '/');
		const lastSlashIndex = normalizedPath.lastIndexOf('/');

		if (lastSlashIndex === -1) {
			return {
				directory: '',
				filename: normalizedPath
			};
		}

		return {
			directory: normalizedPath.slice(0, lastSlashIndex),
			filename: normalizedPath.slice(lastSlashIndex + 1)
		};
	}

	async function handleSelectSave(saveType: SaveType) {
		await goto('/loading');
		send(MessageType.SELECT_SAVE, {
			type: saveType,
			local: isDesktopMode
		});
	}

	$effect(() => {
		if (!isDesktopMode) {
			goto('/upload');
		}
	});
</script>

{#snippet pickYourPoison(size: 'sm' | 'lg' = 'lg')}
	{@const morpheusSize = size === 'lg' ? 'h-[600px]' : 'h-[400px]'}
	{@const inset = size === 'lg' ? 'inset-8' : 'inset-4'}
	{@const offset = size === 'lg' ? 'top-[420px]' : 'top-[280px]'}
	{@const pillSize = size === 'lg' ? 'h-24 w-24' : 'h-16 w-16'}
	<div class="relative flex flex-col">
		<div class="relative">
			<img src={morpheus} alt="Choice" class={cn('w-auto', morpheusSize)} />
			<div class={cn('absolute flex items-center justify-between px-12', inset, offset)}>
				<Tooltip
					popupClass="p-0 bg-surface-600 max-w-96"
					rounded="rounded-none"
					position="left"
					useArrow={false}
				>
					<button
						class={cn('rounded-full bg-transparent transition-transform hover:scale-110', pillSize)}
						onclick={() => handleSelectSave('steam')}
					>
						{#if steamIcon}
							{@html steamIcon}
						{:else}
							Steam
						{/if}
					</button>
					{#snippet popup()}
						<div class="flex flex-col p-4">
							<h4 class="h4">Steam</h4>
							<p>Find and select your Level.sav file.</p>
						</div>
					{/snippet}
				</Tooltip>
				
				<Tooltip
					popupClass="p-0 bg-surface-600 max-w-96"
					rounded="rounded-none"
					position="top"
					useArrow={false}
				>
					<button
						class={cn(
							'bg-blue-500 rounded-full transition-transform hover:scale-110',
							pillSize
						)}
						onclick={() => goto('/sftp')}
					>
						SFTP
					</button>
					{#snippet popup()}
						<div class="flex flex-col p-4">
							<h4 class="h4">SFTP</h4>
							<p>Connect to your server via SFTP</p>
						</div>
					{/snippet}
				</Tooltip>

				<Tooltip
					popupClass="p-0 bg-surface-600 max-w-96"
					rounded="rounded-none"
					position="right"
					useArrow={false}
				>
					<button
						class={cn(
							'-rotate-90 rounded-full bg-transparent transition-transform hover:scale-110',
							pillSize
						)}
						onclick={() => handleSelectSave('gamepass')}
					>
						{#if xboxIcon}
							{@html xboxIcon}
						{:else}
							Xbox
						{/if}
					</button>
					{#snippet popup()}
						<div class="flex flex-col p-4">
							<h4 class="h4">XBOX Game Pass</h4>
							<p>Find and select your container.index file</p>
						</div>
					{/snippet}
				</Tooltip>
			</div>
		</div>
	</div>
{/snippet}

<div class="flex h-full w-full flex-col items-center justify-center space-y-4">
	{#if appState.saveFile && appState.playerSaveFiles}
		{@const { directory, filename } = splitPath(appState.saveFile.name)}
		{@render pickYourPoison('sm')}
		<Card class="min-w-96">
			<div class="grid grid-cols-[auto_1fr]">
				<span class="font-bold">World Name:</span>
				<span class="text-end"> {appState.saveFile.world_name}</span>
				<span class="font-bold">Players:</span>
				<span class="text-end"> {appState.playerSaveFiles.length}</span>
				<span class="font-bold">Pals:</span>
				<span class="text-end"> {totalPals}</span>
				<span class="font-bold">Guilds:</span>
				<span class="text-end"> {Object.values(appState.guilds).length}</span>
				<span class="font-bold">Bases:</span>
				<span class="text-end"> {totalBases}</span>
			</div>
		</Card>
	{:else if appState.gamepassSaves && Object.keys(appState.gamepassSaves).length > 0}
		<GamepassSaveList bind:saves={appState.gamepassSaves} />
	{:else}
		<div class="relative flex h-full w-full items-center justify-center">
			{@render pickYourPoison()}
		</div>
	{/if}
</div>
