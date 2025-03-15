<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { Combobox, Tooltip, TooltipButton } from '$components/ui';
	import { buildingsData } from '$lib/data';
	import { getAppState, getModalState } from '$states';
	import {
		MessageType,
		type Base,
		type CharacterContainer,
		type Guild,
		type ItemContainer,
		type Pal,
		type Player
	} from '$types';
	import { Switch, Tabs } from '@skeletonlabs/skeleton-svelte';
	import { Eye, EyeOff, RefreshCcw, Trash } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { JSONEditor, type ContextMenuItem } from 'svelte-jsoneditor';
	import { send, sendAndWait } from '$lib/utils/websocketUtils';

	type JsonContent = {
		content: { text: string };
	};

	type RawDataType = 'guild' | 'base' | 'player' | 'pal' | 'item_container' | 'character_container';
	const appState = getAppState();
	const modal = getModalState();

	let jsons: Record<RawDataType, JsonContent> = $state({
		guild: { content: { text: '' } },
		base: { content: { text: '' } },
		player: { content: { text: '' } },
		pal: { content: { text: '' } },
		item_container: { content: { text: '' } },
		character_container: { content: { text: '' } }
	});
	let activePage: RawDataType = $state('guild');

	let guild: Guild | undefined = $state(undefined);
	let selectedGuildId: string = $state('');
	let guildRawData: Record<string, any> | undefined = $state(undefined);

	let base: Base | undefined = $state(undefined);
	let selectedBaseId: string = $state('');
	let baseRawData: Record<string, any> | undefined = $state(undefined);

	let player: Player | undefined = $state(undefined);
	let selectedPlayerId: string = $state('');
	let playerRawData: Record<string, any> | undefined = $state(undefined);

	let pal: Pal | undefined = $state(undefined);
	let selectedPalId: string = $state('');
	let palRawData: Record<string, any> | undefined = $state(undefined);

	let itemContainer: ItemContainer | undefined = $state(undefined);
	let selectedItemContainerId: string = $state('');
	let itemContainerRawData: Record<string, any> | undefined = $state(undefined);

	let characterContainer: CharacterContainer | undefined = $state(undefined);
	let selectedCharacterContainerId: string = $state('');
	let characterContainerRawData: Record<string, any> | undefined = $state(undefined);

	const guildSelectOptions = $derived.by(() => {
		return Object.values(appState.guilds).map((guild) => ({
			label: guild.name,
			value: guild.id
		}));
	});

	const baseSelectOptions = $derived.by(() => {
		if (!guild || !(guild as Guild).bases) return [];
		let options: { [key: string]: string } = {};
		for (const baseId of Object.keys(guild.bases)) {
			const base = guild.bases[baseId];
			if (base) {
				options[base.id] = base.id;
			}
		}
		return Object.entries(options).map(([id, name]) => ({
			label: name,
			value: id
		}));
	});

	const playerSelectOptions = $derived.by(() => {
		if (!guild || !(guild as Guild).players)
			return Object.values(appState.players).map((player) => ({
				label: player.nickname,
				value: player.uid
			}));
		let options: { [key: string]: string } = {};
		for (const playerId of guild.players) {
			const player = appState.players[playerId];
			if (player) {
				options[player.uid] = player.nickname;
			}
		}
		return Object.entries(options).map(([uid, nickname]) => ({
			label: nickname,
			value: uid
		}));
	});

	const palSelectOptions = $derived.by(() => {
		let options: { [key: string]: string } = {};
		if (base && (base as Base).pals) {
			for (const palId of Object.keys(base.pals)) {
				const pal = base.pals[palId];
				if (pal) {
					options[pal.instance_id] = pal.nickname || pal.name || pal.character_id;
				}
			}
			return Object.entries(options).map(([id, name]) => ({
				label: name,
				value: id
			}));
		}

		if (player && (player as Player).pals) {
			for (const palId of Object.keys(player.pals!)) {
				const pal = player.pals![palId];
				if (pal) {
					options[pal.instance_id] = pal.nickname || pal.name || pal.character_id;
				}
			}
			return Object.entries(options).map(([id, name]) => ({
				label: name,
				value: id
			}));
		}
		return [];
	});

	const characterContainerSelectOptions = $derived.by(() => {
		let options: { [key: string]: string } = {};
		if (base) {
			options[base.container_id] = 'Base Container';
			return Object.entries(options).map(([id, name]) => ({
				label: name,
				value: id
			}));
		}
		if (player) {
			options[player.pal_box_id] = 'Pal Box';
			options[player.otomo_container_id] = 'Party';
			return Object.entries(options).map(([id, name]) => ({
				label: name,
				value: id
			}));
		}
		return [];
	});

	const itemContainerSelectOptions = $derived.by(() => {
		let options: { [key: string]: string } = {};
		if (base) {
			for (const [id, container] of Object.entries(base.storage_containers!)) {
				if (container) {
					const buildingData = buildingsData.buildings[container.key];
					options[id] = buildingData?.localized_name || container.key;
				}
			}
			return Object.entries(options).map(([id, name]) => ({
				label: name,
				value: id
			}));
		}
		if (player) {
			options['common'] = 'Common';
			options['essential'] = 'Key Items';
			options['weapon'] = 'Weapon';
			options['armor'] = 'Armor';
			options['food'] = 'Food';
			return Object.entries(options).map(([id, name]) => ({
				label: name,
				value: id
			}));
		}
		return [];
	});

	function setJson(tab: RawDataType, data: any) {
		if (!data) return;
		jsons[tab].content.text = JSON.stringify(data, null, 2);
		activePage = tab;
	}

	async function handleSelectPlayer(selectedPlayerId: string | number) {
		if (!selectedPlayerId) return;
		player = Object.values(appState.players).find((p) => p.uid == selectedPlayerId);
		setJson('player', player);
	}

	function handleSelectGuild(selectedGuildId: string | number, reset = true) {
		if (!selectedGuildId) return;
		guild = Object.values(appState.guilds).find((g) => g.id == selectedGuildId);
		setJson('guild', guild);
		if (!reset) return;
		base = player = pal = itemContainer = characterContainer = undefined;
		selectedBaseId =
			selectedPlayerId =
			selectedPalId =
			selectedItemContainerId =
			selectedCharacterContainerId =
				'';
	}

	function handleSelectBase(selectedBaseId: string | number, reset = true) {
		if (!selectedBaseId) return;
		base = guild?.bases[selectedBaseId];
		setJson('base', base);
		if (!reset) return;
		player = pal = itemContainer = characterContainer = undefined;
		selectedPlayerId = selectedPalId = selectedItemContainerId = selectedCharacterContainerId = '';
	}

	function searchAllPals(palId: string) {
		for (const player of Object.values(appState.players)) {
			if (player.pals?.[palId]) {
				handleSelectPlayer(player.uid);
				handleSelectGuild(player.guild_id, false);
				return player.pals[palId];
			}
		}
		for (const base of Object.values(appState.guilds).flatMap((g) => Object.values(g.bases))) {
			if (base.pals?.[palId]) {
				selectedBaseId = base.id;
				handleSelectBase(base.id, false);
				return base.pals[palId];
			}
		}
	}

	function handleSelectPal(selectedPalId: string | number) {
		if (!selectedPalId) return;
		pal =
			base?.pals[selectedPalId] ||
			player?.pals?.[selectedPalId] ||
			searchAllPals(selectedPalId as string);
		setJson('pal', pal);
	}

	function handleSelectCharacterContainer(selectedCharacterContainerId: string | number) {
		if (base) {
			characterContainer = base.pal_container;
		} else if (player) {
			characterContainer =
				selectedCharacterContainerId === player?.pal_box_id ? player?.pal_box : player?.party;
		}
		setJson('character_container', characterContainer);
	}

	function handleSelectItemContainer(selectedItemContainerId: string | number) {
		if (base) {
			itemContainer = base.storage_containers?.[selectedItemContainerId];
		} else if (player) {
			switch (selectedItemContainerId) {
				case 'common':
					itemContainer = player?.common_container;
					break;
				case 'essential':
					itemContainer = player?.essential_container;
					break;
				case 'weapon':
					itemContainer = player?.weapon_load_out_container;
					break;
				case 'armor':
					itemContainer = player?.player_equipment_armor_container;
					break;
				case 'food':
					itemContainer = player?.food_equip_container;
					break;
			}
		}
		setJson('item_container', itemContainer);
	}

	async function fetchGuildRawData(guild_id: any) {
		const res = await sendAndWait(MessageType.GET_RAW_DATA, { guild_id });
		if (!res) return;
		guildRawData = res;
		setJson('guild', guildRawData);
	}

	async function fetchBaseRawData(base_id: any) {
		const res = await sendAndWait(MessageType.GET_RAW_DATA, { base_id });
		if (!res) return;
		baseRawData = res;
		setJson('base', baseRawData);
	}

	async function fetchPlayerRawData(player_id: any) {
		const res = await sendAndWait(MessageType.GET_RAW_DATA, { player_id });
		if (!res) return;
		console.log('Res', res);
		playerRawData = res;
		setJson('player', playerRawData);
	}

	async function fetchPalRawData(pal_id: any) {
		const res = await sendAndWait(MessageType.GET_RAW_DATA, { pal_id });
		if (!res) return;
		palRawData = res;
		setJson('pal', palRawData);
	}

	async function fetchCharacterContainerRawData(character_container_id: any) {
		const res = await sendAndWait(MessageType.GET_RAW_DATA, { character_container_id });
		if (!res) return;
		characterContainerRawData = res;
		setJson('character_container', characterContainerRawData);
	}

	async function handleGetRawData(type: RawDataType) {
		switch (type) {
			case 'guild':
				if (!guild) return;
				await fetchGuildRawData(guild.id);
				break;
			case 'base':
				if (!base) return;
				await fetchBaseRawData(base.id);
				break;
			case 'player':
				if (!player) return;
				await fetchPlayerRawData(player.uid);
				break;
			case 'pal':
				if (!pal) return;
				await fetchPalRawData(pal.instance_id);
				break;
			// case 'item_container':
			// 	if (!itemContainer) return;
			// 	await fetchPalRawData(itemContainer.id);
			// 	break;
			case 'character_container':
				if (!characterContainer) return;
				await fetchCharacterContainerRawData(characterContainer.id);
				break;
		}
	}

	function onRenderContextMenu(items: ContextMenuItem[]) {
		console.log(items);
		return items;
	}

	function formatTabTitle(tab: RawDataType) {
		// replace underscores with spaces and capitalize the first letter of each word
		return tab
			.replace(/_/g, ' ')
			.split(' ')
			.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
			.join(' ');
	}

	function selectClass(tab: RawDataType) {
		return activePage === tab ? 'bg-surface-900 ring ring-secondary-500' : 'bg-surface-900';
	}

	onMount(() => {
		const { url } = page;
		const params = new URLSearchParams(url.search);
		selectedGuildId = params.get('guildId') || '';
		selectedPlayerId = params.get('playerId') || '';
		selectedBaseId = params.get('baseId') || '';
		selectedPalId = params.get('palId') || '';
		selectedItemContainerId = params.get('itemContainerId') || '';
		selectedCharacterContainerId = params.get('characterContainerId') || '';
		if (selectedGuildId) {
			handleSelectGuild(selectedGuildId, false);
		}
		if (selectedPlayerId) {
			handleSelectPlayer(selectedPlayerId);
		}
		if (selectedBaseId) {
			handleSelectBase(selectedBaseId, false);
		}
		if (selectedPalId) {
			handleSelectPal(selectedPalId);
		}
		if (selectedItemContainerId) {
			handleSelectItemContainer(selectedItemContainerId);
		}
		if (selectedCharacterContainerId) {
			handleSelectCharacterContainer(selectedCharacterContainerId);
		}
	});

	function handleReset() {
		guild = undefined;
		base = undefined;
		player = undefined;
		pal = undefined;
		itemContainer = undefined;
		guildRawData = undefined;
		baseRawData = undefined;
		playerRawData = undefined;
		palRawData = undefined;
		itemContainerRawData = undefined;
		characterContainerRawData = undefined;

		jsons.guild.content.text = '';
		jsons.base.content.text = '';
		jsons.player.content.text = '';
		jsons.pal.content.text = '';
		jsons.item_container.content.text = '';
		jsons.character_container.content.text = '';
		activePage = 'guild';
		selectedGuildId = '';
		selectedPlayerId = '';
		selectedBaseId = '';
		selectedPalId = '';
		selectedItemContainerId = '';
	}

	async function handleDeletePlayer() {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Player',
			message: 'Are you sure you want to delete this player? This action cannot be undone.',
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (confirmed) {
			send(MessageType.DELETE_PLAYER, { player_id: selectedPlayerId, origin: 'debug' });
			goto('/loading');
		}
	}

	async function handleDeleteGuild() {
		console.log('handleDeleteGuild', selectedGuildId);
		const confirmed = await modal.showConfirmModal({
			title: 'Delete Guild',
			message: 'Are you sure you want to delete this guild? This action cannot be undone.',
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (confirmed) {
			send(MessageType.DELETE_GUILD, {
				guild_id: selectedGuildId,
				origin: 'debug'
			});
			goto('/loading');
		}
	}
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2 p-2">
	<div class="flex flex-col">
		<button class="btn preset-filled-primary-500 mb-2 flex w-full" onclick={handleReset}>
			<span class="font-medium"> Reset </span>
			<RefreshCcw size="20" />
		</button>

		<div class="flex items-center gap-2">
			<Combobox
				label="Guild"
				options={guildSelectOptions}
				bind:value={selectedGuildId}
				placeholder="Select Guild"
				onChange={handleSelectGuild}
				selectClass={selectClass('guild')}
			/>
			<Tooltip label="Toggle Raw">
				<Switch
					defaultChecked={false}
					onCheckedChange={(e) => {
						e.checked ? handleGetRawData('guild') : setJson('guild', guild!);
					}}
					disabled={!guild}
					compact
				>
					{#snippet inactiveChild()}
						<EyeOff size="20" />
					{/snippet}
					{#snippet activeChild()}
						<Eye size="20" />
					{/snippet}
				</Switch>
			</Tooltip>
			<TooltipButton
				popupLabel="Delete Guild"
				buttonClass="bg-surface-900 rounded-full hover:bg-red-500/50"
				onclick={handleDeleteGuild}
				disabled={!guild}
			>
				<Trash size="20" />
			</TooltipButton>
		</div>
		{#if baseSelectOptions.length > 0}
			<div class="flex items-center gap-2">
				<Combobox
					label="Base"
					options={baseSelectOptions}
					bind:value={selectedBaseId}
					placeholder="Select Base"
					onChange={handleSelectBase}
					selectClass={selectClass('base')}
				/>
				<Tooltip label="Toggle Raw">
					<Switch
						defaultChecked={false}
						onCheckedChange={(e) => {
							e.checked ? handleGetRawData('base') : setJson('base', base!);
						}}
						disabled={!base}
						compact
					>
						{#snippet inactiveChild()}
							<EyeOff size="20" />
						{/snippet}
						{#snippet activeChild()}
							<Eye size="20" />
						{/snippet}
					</Switch>
				</Tooltip>
			</div>
		{/if}

		<div class="flex items-center gap-2">
			<Combobox
				label="Player"
				options={playerSelectOptions}
				bind:value={selectedPlayerId}
				placeholder={guild ? 'Select Guild Player' : 'Select Player'}
				onChange={handleSelectPlayer}
				selectClass={selectClass('player')}
			/>
			<Tooltip label="Toggle Raw">
				<Switch
					defaultChecked={false}
					onCheckedChange={(e) => {
						e.checked ? handleGetRawData('player') : setJson('player', player!);
					}}
					disabled={!player}
					compact
				>
					{#snippet inactiveChild()}
						<EyeOff size="20" />
					{/snippet}
					{#snippet activeChild()}
						<Eye size="20" />
					{/snippet}
				</Switch>
			</Tooltip>
			<TooltipButton
				popupLabel="Delete Player"
				buttonClass="bg-surface-900 rounded-full hover:bg-red-500/50"
				onclick={handleDeletePlayer}
				disabled={!player}
			>
				<Trash size="20" />
			</TooltipButton>
		</div>

		{#if palSelectOptions.length > 0}
			<div class="flex items-center gap-2">
				<Combobox
					label="Pal"
					options={palSelectOptions}
					bind:value={selectedPalId}
					placeholder={base ? 'Select Base Pal' : 'Select Player Pal'}
					onChange={handleSelectPal}
					selectClass={selectClass('pal')}
				/>
				<Tooltip label="Toggle Raw">
					<Switch
						defaultChecked={false}
						onCheckedChange={(e) => {
							e.checked ? handleGetRawData('pal') : setJson('pal', pal!);
						}}
						disabled={!pal}
						compact
					>
						{#snippet inactiveChild()}
							<EyeOff size="20" />
						{/snippet}
						{#snippet activeChild()}
							<Eye size="20" />
						{/snippet}
					</Switch>
				</Tooltip>
			</div>
		{/if}

		{#if characterContainerSelectOptions.length > 0}
			<div class="flex items-center gap-2">
				<Combobox
					label="Character Container"
					options={characterContainerSelectOptions}
					bind:value={selectedCharacterContainerId}
					placeholder={base
						? 'Select Base Character Container'
						: 'Select Player Character Container'}
					onChange={handleSelectCharacterContainer}
					selectClass={selectClass('character_container')}
				/>
				<Tooltip label="Toggle Raw">
					<Switch
						defaultChecked={false}
						onCheckedChange={(e) => {
							e.checked
								? handleGetRawData('character_container')
								: setJson('character_container', characterContainer!);
						}}
						disabled={!characterContainer}
						compact
					>
						{#snippet inactiveChild()}
							<EyeOff size="20" />
						{/snippet}
						{#snippet activeChild()}
							<Eye size="20" />
						{/snippet}
					</Switch>
				</Tooltip>
			</div>
		{/if}

		{#if itemContainerSelectOptions.length > 0}
			<div class="flex items-center gap-2">
				<Combobox
					label="Item Container"
					options={itemContainerSelectOptions}
					bind:value={selectedItemContainerId}
					placeholder={base ? 'Select Base Item Container' : 'Select Player Item Container'}
					onChange={handleSelectItemContainer}
					selectClass={selectClass('item_container')}
				/>
				<Tooltip label="Toggle Raw">
					<Switch
						defaultChecked={false}
						onCheckedChange={(e) => {
							e.checked
								? handleGetRawData('item_container')
								: setJson('item_container', itemContainer!);
						}}
						disabled
						compact
					>
						{#snippet inactiveChild()}
							<EyeOff size="20" />
						{/snippet}
						{#snippet activeChild()}
							<Eye size="20" />
						{/snippet}
					</Switch>
				</Tooltip>
			</div>
		{/if}
	</div>

	<Tabs value={activePage} onValueChange={(e) => (activePage = e.value as RawDataType)}>
		{#snippet list()}
			{#each Object.keys(jsons) as key}
				<Tabs.Control
					value={key}
					stateActive="border-b-secondary-500 opacity-100 text-secondary-500"
				>
					{formatTabTitle(key as RawDataType)}
				</Tabs.Control>
			{/each}
		{/snippet}
		{#snippet content()}
			{#each Object.keys(jsons) as key}
				<Tabs.Panel value={key}>
					<div class="editor-wrapper">
						<JSONEditor bind:content={jsons[key as RawDataType].content} {onRenderContextMenu} />
					</div>
				</Tabs.Panel>
			{/each}
		{/snippet}
	</Tabs>
</div>
