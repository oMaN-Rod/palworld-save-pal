<script lang="ts">
	import { page } from '$app/state';
	import { Combobox, Tooltip } from '$components/ui';
	import { buildingsData } from '$lib/data';
	import { getAppState, getSocketState } from '$states';
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
	import { Eye, EyeOff, RefreshCcw } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { JSONEditor, type ContextMenuItem, type JSONEditorSelection } from 'svelte-jsoneditor';

	type JsonContent = {
		content: { text: string };
	};

	type RawDataType = 'guild' | 'base' | 'player' | 'pal' | 'item_container' | 'character_container';

	const ws = getSocketState();
	const appState = getAppState();

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

	async function fetchGuildRawData(message: Record<string, any>) {
		const res = await ws.sendAndWait(message);
		if (!res || !res.data) return;
		guildRawData = res.data;
		setJson('guild', guildRawData);
	}

	async function fetchBaseRawData(message: Record<string, any>) {
		const res = await ws.sendAndWait(message);
		if (!res || !res.data) return;
		baseRawData = res.data;
		setJson('base', baseRawData);
	}

	async function fetchPlayerRawData(message: Record<string, any>) {
		const res = await ws.sendAndWait(message);
		if (!res || !res.data) return;
		playerRawData = res.data;
		setJson('player', playerRawData);
	}

	async function fetchPalRawData(message: Record<string, any>) {
		const res = await ws.sendAndWait(message);
		if (!res || !res.data) return;
		palRawData = res.data;
		setJson('pal', palRawData);
	}

	async function fetchCharacterContainerRawData(message: Record<string, any>) {
		const res = await ws.sendAndWait(message);
		if (!res || !res.data) return;
		characterContainerRawData = res.data;
		setJson('character_container', characterContainerRawData);
	}

	async function handleGetRawData(type: RawDataType) {
		let message = {
			type: MessageType.GET_RAW_DATA,
			data: {}
		};
		switch (type) {
			case 'guild':
				if (!guild) return;
				message.data = {
					guild_id: guild.id
				};
				await fetchGuildRawData(message);
				break;
			case 'base':
				if (!base) return;
				message.data = {
					base_id: base.id
				};
				await fetchBaseRawData(message);
				break;
			case 'player':
				if (!player) return;
				message.data = {
					player_id: player.uid
				};
				await fetchPlayerRawData(message);
				break;
			case 'pal':
				if (!pal) return;
				message.data = {
					pal_id: pal.instance_id
				};
				await fetchPalRawData(message);
				break;
			case 'item_container':
				if (!itemContainer) return;
				message.data = {
					item_container_id: itemContainer.id
				};
				await fetchPalRawData(message);
				break;
			case 'character_container':
				if (!characterContainer) return;
				message.data = {
					character_container_id: characterContainer.id
				};
				await fetchCharacterContainerRawData(message);
				break;
		}
	}

	function onRenderContextMenu(
		items: ContextMenuItem[],
		context: {
			mode: 'tree' | 'text' | 'table';
			modal: boolean;
			readOnly: boolean;
			selection: JSONEditorSelection | undefined;
		}
	) {
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
		</div>
		{#if baseSelectOptions.length > 0}
			<div class="flex items-center gap-2">
				<Combobox
					label="Base"
					options={baseSelectOptions}
					bind:value={selectedBaseId}
					placeholder="Select Base"
					onChange={handleSelectBase}
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
		</div>

		{#if palSelectOptions.length > 0}
			<div class="flex items-center gap-2">
				<Combobox
					label="Pal"
					options={palSelectOptions}
					bind:value={selectedPalId}
					placeholder={base ? 'Select Base Pal' : 'Select Player Pal'}
					onChange={handleSelectPal}
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
				/>
				<Tooltip label="Toggle Raw">
					<Switch
						defaultChecked={false}
						onCheckedChange={(e) => {
							e.checked
								? handleGetRawData('item_container')
								: setJson('item_container', itemContainer!);
						}}
						disabled={!itemContainer}
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
				<Tabs.Control value={key}>
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
