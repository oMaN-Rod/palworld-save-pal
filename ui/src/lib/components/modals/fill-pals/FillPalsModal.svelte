<script lang="ts">
	import { Card, List, Tooltip } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import { getAppState } from '$states';
	import { EntryState, MessageType, type Pal, type PalData, type PresetProfile } from '$types';
	import { applyPalPreset, assetLoader, canBeAlpha, canBeLucky, formatNickname } from '$utils';
	import { sendAndWait } from '$utils/websocketUtils';
	import NumberFlow from '@number-flow/svelte';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import { X, Check, Trash, Lock } from 'lucide-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';

	let {
		title,
		target = 'pal-box',
		closeModal
	} = $props<{
		title: string;
		target?: 'pal-box' | 'dps';
		closeModal: (value: boolean | null) => void;
	}>();

	interface ExtendedPresetProfile extends PresetProfile {
		id: string;
	}

	let addNormalPals = $state(true);
	let addLuckyPals = $state(true);
	let addAlphaPals = $state(true);
	let addBossPals = $state(true);
	let addPredatorPals = $state(true);
	let addRaidPals = $state(true);
	let addSummonPals = $state(true);
	let addOilRigPals = $state(true);
	let addHumanPals = $state(true);
	let isBusy = $state(false);
	let count = $state(0);
	let selectedPresets: ExtendedPresetProfile[] = $state([]);

	const specialCases = ['PREDATOR_', 'RAID_', 'GYM_', 'SUMMON_', '_OILRIG'];
	const appState = getAppState();

	const palPresets: ExtendedPresetProfile[] = $derived.by(() => {
		return Object.entries(presetsData.presetProfiles)
			.filter(([_, profile]) => profile.type === 'pal_preset')
			.map(([id, preset]) => ({
				id,
				...preset
			}))
			.sort((a, b) => a.name.localeCompare(b.name));
	});
	const palBoxPals = $derived(
		Object.values(appState.selectedPlayer!.pals || {}).filter(
			(p) => p.storage_id != appState.selectedPlayer!.otomo_container_id
		) as Pal[]
	);
	const totalPalBoxPals = $derived(
		Object.values(palBoxPals).filter(
			(p) => p.storage_id != appState.selectedPlayer!.otomo_container_id
		).length
	);
	const totalDpsPals = $derived(Object.values(appState.selectedPlayer!.dps || {}).length);
	const availablePalBoxSlots = $derived(
		target === 'pal-box' ? 960 - totalPalBoxPals : 9600 - totalDpsPals
	);

	const humanPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => !p[1].is_pal)
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const normalPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => {
				return (
					!specialCases.some((substring) => p[0].toUpperCase().includes(substring)) && p[1].is_pal
				);
			})
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const predatorPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => p[0].toUpperCase().includes('PREDATOR_'))
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const raidPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => p[0].toUpperCase().includes('RAID_'))
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const bossPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => p[0].toUpperCase().includes('GYM_'))
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const summonPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => p[0].toUpperCase().includes('SUMMON_'))
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const oilRigPals = $derived(
		Object.entries(palsData.pals)
			.filter((p) => !p[1].disabled)
			.filter((p) => p[0].toUpperCase().includes('OILRIG_'))
			.sort((a, b) => {
				const indexA = (a[1] as PalData)?.pal_deck_index ?? Infinity;
				const indexB = (b[1] as PalData)?.pal_deck_index ?? Infinity;
				return indexA - indexB;
			})
	);
	const totalRequiredSlots = $derived.by(() => {
		let count = 0;
		if (addNormalPals) count += normalPals.length;
		if (addLuckyPals) count += normalPals.filter((p) => canBeLucky(p[0])[1]).length;
		if (addAlphaPals) count += normalPals.filter((p) => canBeAlpha(p[0])[1]).length;
		if (addBossPals) count += bossPals.length;
		if (addPredatorPals) count += predatorPals.length;
		if (addRaidPals) count += raidPals.length;
		if (addSummonPals) count += summonPals.length;
		if (addOilRigPals) count += oilRigPals.length;
		if (addHumanPals) count += humanPals.length;
		return count;
	});
	const canAddPals = $derived.by(() => {
		if (availablePalBoxSlots <= 0) {
			return false;
		}
		return totalRequiredSlots <= availablePalBoxSlots;
	});

	const message = $derived.by(() => {
		if (target === 'pal-box') {
			if (totalPalBoxPals === 0) {
				return `You currently have no pals in your pal box. Are you sure you want to fill it?`;
			}
			return !isBusy
				? `You currently have ${availablePalBoxSlots} available slots in your pal box..`
				: '';
		}
	});

	const handleApplyPalPreset = (pal: Record<string, any>) => {
		if (!selectedPresets || selectedPresets.length === 0) {
			return;
		}
		const palData = palsData.getByKey(pal.character_key);
		const palProfile =
			selectedPresets.filter(
				(p) => p.pal_preset?.lock && p.pal_preset?.character_id === pal.character_id
			)[0] || undefined;
		const elementProfile =
			selectedPresets.filter(
				(p) =>
					p.pal_preset?.lock_element &&
					palData?.element_types.includes(p.pal_preset?.element as any)
			)[0] || undefined;
		const defaultProfile =
			selectedPresets.filter((p) => !p.pal_preset?.lock && !p.pal_preset?.lock_element)[0] ||
			undefined;
		const profile = palProfile || elementProfile || defaultProfile;
		applyPalPreset(pal as Pal, profile, appState.selectedPlayer!);
	};

	async function addPal(character_id: string, nickname: string, name: string) {
		let res: { player_uid: string; pal: Pal; index: number } | undefined = undefined;
		if (target === 'pal-box') {
			res = await sendAndWait(MessageType.ADD_PAL, {
				player_id: appState.selectedPlayer!.uid,
				character_id,
				nickname,
				container_id: appState.selectedPlayer!.pal_box_id
			});
		} else {
			res = await sendAndWait(MessageType.ADD_DPS_PAL, {
				player_id: appState.selectedPlayer!.uid,
				character_id,
				nickname
			});
		}

		if (!res) {
			console.error(`Failed to add pal ${character_id} for player ${appState.selectedPlayer!.uid}`);
			return;
		}
		res.pal.name = name;
		handleApplyPalPreset(res.pal);
		if (target === 'pal-box') {
			appState.selectedPlayer!.pals![res.pal.instance_id] = res.pal;
		} else {
			if (appState.selectedPlayer!.dps) {
				appState.selectedPlayer!.dps[res.index] = res.pal;
			} else {
				appState.selectedPlayer!.dps = { [res.index]: res.pal };
			}
		}

		count++;
		return res.pal;
	}

	async function createBasePalVariants() {
		for (const [character_id, palData] of normalPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			if (addNormalPals) {
				await addPal(character_id, nickname, palData.localized_name || character_id);
			}
			if (addLuckyPals) {
				if (!canBeLucky(character_id)[1]) {
					continue;
				}
				const pal = await addPal(character_id, nickname, palData.localized_name || character_id);
				if (!pal) {
					console.error(`Failed to add lucky pal for ${character_id}`);
					continue;
				}
				pal.is_lucky = true;
				pal.state = EntryState.MODIFIED;
			}
			if (addAlphaPals) {
				if (!canBeAlpha(character_id)[1]) {
					continue;
				}
				const pal = await addPal(character_id, nickname, palData.localized_name || character_id);
				if (!pal) {
					console.error(`Failed to add alpha pal for ${character_id}`);
					continue;
				}
				pal.is_boss = true;
				pal.state = EntryState.MODIFIED;
			}
		}
	}

	async function createTowerBosses() {
		if (!addBossPals) {
			return;
		}
		for (const [character_id, palData] of bossPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			await addPal(character_id, nickname, palData.localized_name || character_id);
		}
	}

	async function createPredatorPals() {
		if (!addPredatorPals) {
			return;
		}
		for (const [character_id, palData] of predatorPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			await addPal(character_id, nickname, palData.localized_name || character_id);
		}
	}

	async function createRaidPals() {
		if (!addRaidPals) {
			return;
		}
		for (const [character_id, palData] of raidPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			await addPal(character_id, nickname, palData.localized_name || character_id);
		}
	}

	async function createSummonPals() {
		if (!addSummonPals) {
			return;
		}
		for (const [character_id, palData] of summonPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			await addPal(character_id, nickname, palData.localized_name || character_id);
		}
	}

	async function createOilRigPals() {
		if (!addOilRigPals) {
			return;
		}
		for (const [character_id, palData] of oilRigPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			await addPal(character_id, nickname, palData.localized_name || character_id);
		}
	}

	async function createHumanPals() {
		if (!addHumanPals) {
			return;
		}
		for (const [character_id, palData] of humanPals) {
			const nickname = formatNickname(
				palData.localized_name || character_id,
				appState.settings.new_pal_prefix
			);
			await addPal(character_id, nickname, palData.localized_name || character_id);
		}
	}

	async function handleConfirm() {
		isBusy = true;
		await createBasePalVariants();
		await createPredatorPals();
		await createRaidPals();
		await createSummonPals();
		await createOilRigPals();
		await createHumanPals();
		await createTowerBosses();
		closeModal(true);
	}

	function handleCancel() {
		closeModal(false);
	}
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<div class="mb-2 flex flex-col">
		<span>{message}</span>
	</div>

	<div class="flex space-x-2">
		{#if isBusy}
			<div class="flex w-full items-center justify-center space-x-2">
				<span class="text-surface-500 text-2xl">created</span>
				<span class="text-4xl font-bold"><NumberFlow value={count} /></span>
				<span class="text-surface-500 text-2xl"> of </span>
				<span class="text-surface-500 text-2xl">{totalRequiredSlots}</span>
				<span class="text-surface-500 text-2xl"> pals </span>
			</div>
		{:else}
			<div>
				<h6 class="h6">Types</h6>
				<div class="mt-2 grid grid-cols-2 gap-2 overflow-y-auto p-2">
					<Tooltip
						label={`Add ${normalPals.length} Normal Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Normal"
							checked={addNormalPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addNormalPals = mode.checked;
							}}
						/>
						<span>Normal</span>
					</Tooltip>
					<Tooltip
						label={`Add ${normalPals.length} Lucky Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Normal"
							checked={addLuckyPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addLuckyPals = mode.checked;
							}}
						/>
						<span>Lucky</span>
					</Tooltip>
					<Tooltip
						label={`Add ${normalPals.length} Alpha Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Alpha"
							checked={addAlphaPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addAlphaPals = mode.checked;
							}}
						/>
						<span>Alpha</span>
					</Tooltip>
					<Tooltip
						label={`Add ${bossPals.length} Boss Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Boss"
							checked={addBossPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addBossPals = mode.checked;
							}}
						/>
						<span>Boss</span>
					</Tooltip>
					<Tooltip
						label={`Add ${predatorPals.length} Predator Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Predator"
							checked={addPredatorPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addPredatorPals = mode.checked;
							}}
						/>
						<span>Predator</span>
					</Tooltip>
					<Tooltip
						label={`Add ${raidPals.length} Raid Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Raid"
							checked={addRaidPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addRaidPals = mode.checked;
							}}
						/>
						<span>Raid</span>
					</Tooltip>
					<Tooltip
						label={`Add ${summonPals.length} Summon Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Summon"
							checked={addSummonPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addSummonPals = mode.checked;
							}}
						/>
						<span>Summon</span>
					</Tooltip>
					<Tooltip
						label={`Add ${oilRigPals.length} Oil Rig Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Oil Rig"
							checked={addOilRigPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addOilRigPals = mode.checked;
							}}
						/>
						<span>Oil Rig</span>
					</Tooltip>
					<Tooltip
						label={`Add ${humanPals.length} Human Pals`}
						baseClass="flex items-center space-x-2"
					>
						<Switch
							name="Human"
							checked={addHumanPals}
							onCheckedChange={(mode: CheckedChangeDetails) => {
								addHumanPals = mode.checked;
							}}
						/>
						<span>Human</span>
					</Tooltip>
				</div>
			</div>
			<Card background="preset-filled-surface-200-800">
				<h6 class="h6">Apply Presets</h6>
				<p class="text-sm">
					Presets precedence is: pal lock ➡️ element lock ➡️ default. Only select one preset per
					pal, element, or default.
				</p>
				{#if palPresets.length > 0}
					<List
						items={palPresets}
						bind:selectedItems={selectedPresets}
						baseClass="bg-surface-900 rounded-md"
						listClass="mt-2 max-h-60 overflow-y-auto bg-surface-900"
						itemClass="border-y border-surface-800"
						idKey="id"
					>
						{#snippet listHeader()}
							<div>
								<span class="font-bold">All</span>
							</div>
						{/snippet}
						{#snippet listItem(preset)}
							<div class="flex items-center space-x-2">
								<span>{preset.name}</span>
								{#if preset.pal_preset?.lock}
									{@const palIcon = assetLoader.loadMenuImage(
										preset.pal_preset.character_id as string
									)}
									<img src={palIcon} alt={preset.pal_preset.character_id} class="ml-2 h-8 w-8" />
									<Lock class="ml-2 h-4 w-4 text-red-500" />
								{/if}
								{#if preset.pal_preset?.lock_element}
									{@const elementData = elementsData.getByKey(preset.pal_preset.element as string)}
									{@const elementIcon = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/${elementData?.badge_icon}.webp`
									)}
									<img src={elementIcon} alt={elementData?.name} class="ml-2 h-6 w-6" />
								{/if}
							</div>
						{/snippet}
						{#snippet listItemActions(preset)}
							<button
								class="btn hover:bg-error-500/25 p-2"
								onclick={() =>
									(selectedPresets = selectedPresets.filter((p) => p.id !== preset.id))}
							>
								<Trash size={16} />
							</button>
						{/snippet}
						{#snippet listItemPopup(preset)}
							<div class="flex items-center space-x-2">
								<span>{preset.name}</span>
								{#if preset.pal_preset?.lock}
									<Lock class="ml-2 h-4 w-4 text-red-500" />
								{/if}
								{#if preset.pal_preset?.lock_element}
									{@const elementData = elementsData.getByKey(preset.pal_preset.element as string)}
									{@const elementIcon = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/${elementData?.badge_icon}.webp`
									)}
									<img src={elementIcon} alt={elementData?.name} class="ml-2 h-4 w-4" />
								{/if}
							</div>
						{/snippet}
					</List>
				{/if}
			</Card>
		{/if}
	</div>

	{#if canAddPals && !isBusy}
		<div class="mt-1 flex w-full justify-end text-sm text-green-500">
			<span class="font-bold">Ready to fill {target} with {totalRequiredSlots} pals.</span>
		</div>
	{:else if !canAddPals && !isBusy}
		<div class="mt-1 flex w-full justify-end text-sm text-red-500">
			<span class="font-bold">
				You cannot fill your {target}, you need {Math.abs(
					availablePalBoxSlots - totalRequiredSlots
				)} more slots.
			</span>
		</div>
	{/if}
	<div class="flex justify-end space-x-4">
		<Tooltip position="bottom" label="Cancel">
			<button
				class="btn preset-filled-secondary hover:preset-tonal-secondary"
				onclick={handleCancel}
			>
				<X size={20} />
				<span>Cancel</span>
			</button>
		</Tooltip>
		<Tooltip position="bottom" label="Confirm">
			<button
				class="btn preset-filled-primary hover:preset-tonal-primary"
				onclick={handleConfirm}
				disabled={!canAddPals || isBusy}
			>
				<Check size={20} />
				<span>Fill</span>
			</button>
		</Tooltip>
	</div>
</Card>
