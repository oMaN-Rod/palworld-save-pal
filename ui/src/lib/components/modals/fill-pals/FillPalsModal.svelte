<script lang="ts">
	import { Button, Card, List, Tooltip } from '$components/ui';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { elementsData, palsData, presetsData } from '$lib/data';
	import { getAppState } from '$states';
	import { EntryState, MessageType, type Pal, type PalData, type PresetProfile } from '$types';
	import { applyPalPreset, assetLoader, canBeAlpha, canBeLucky, formatNickname } from '$utils';
	import { sendAndWait } from '$utils/websocketUtils';
	import NumberFlow from '@number-flow/svelte';
	import { X, Check, Trash, Lock } from 'lucide-svelte';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import PalTypeToggles from './PalTypeToggles.svelte';
	import {
		getNormalPals,
		getHumanPals,
		getPredatorPals,
		getRaidPals,
		getBossPals,
		getSummonPals,
		getOilRigPals
	} from './palFilters';

	let {
		title,
		target = 'pal-box',
		closeModal
	} = $props<{
		title: string;
		target?: 'pal-box' | 'dps' | 'gps';
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

	const normalPals = $derived(getNormalPals());
	const humanPals = $derived(getHumanPals());
	const predatorPals = $derived(getPredatorPals());
	const raidPals = $derived(getRaidPals());
	const bossPals = $derived(getBossPals());
	const summonPals = $derived(getSummonPals());
	const oilRigPals = $derived(getOilRigPals());

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
	const totalGpsPals = $derived(Object.values(appState.gps || {}).length);
	const availablePalBoxSlots = $derived.by(() => {
		switch (target) {
			case 'dps':
				return 9600 - totalDpsPals;
			case 'gps':
				return 960 - totalGpsPals;
			default:
				return 960 - totalPalBoxPals;
		}
	});

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
		if (availablePalBoxSlots <= 0) return false;
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

	const toggles = $derived([
		{ label: m.normal(), count: normalPals.length, checked: addNormalPals, onChange: (v: boolean) => (addNormalPals = v) },
		{ label: m.lucky(), count: normalPals.length, checked: addLuckyPals, onChange: (v: boolean) => (addLuckyPals = v) },
		{ label: m.alpha(), count: normalPals.length, checked: addAlphaPals, onChange: (v: boolean) => (addAlphaPals = v) },
		{ label: m.boss(), count: bossPals.length, checked: addBossPals, onChange: (v: boolean) => (addBossPals = v) },
		{ label: m.predator(), count: predatorPals.length, checked: addPredatorPals, onChange: (v: boolean) => (addPredatorPals = v) },
		{ label: m.raid(), count: raidPals.length, checked: addRaidPals, onChange: (v: boolean) => (addRaidPals = v) },
		{ label: m.summon(), count: summonPals.length, checked: addSummonPals, onChange: (v: boolean) => (addSummonPals = v) },
		{ label: m.oil_rig(), count: oilRigPals.length, checked: addOilRigPals, onChange: (v: boolean) => (addOilRigPals = v) },
		{ label: c.human, count: humanPals.length, checked: addHumanPals, onChange: (v: boolean) => (addHumanPals = v) }
	]);

	const handleApplyPalPreset = (pal: Record<string, any>) => {
		if (!selectedPresets || selectedPresets.length === 0) return;
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
		applyPalPreset(pal as Pal, profile, target === 'gps' ? undefined : appState.selectedPlayer!);
	};

	async function addPal(character_id: string, nickname: string, name: string) {
		let res: { player_uid: string; pal: Pal; index: number } | undefined = undefined;
		switch (target) {
			case 'pal-box':
				res = await sendAndWait(MessageType.ADD_PAL, {
					player_id: appState.selectedPlayer!.uid,
					character_id,
					nickname,
					container_id: appState.selectedPlayer!.pal_box_id
				});
				break;
			case 'dps':
				res = await sendAndWait(MessageType.ADD_DPS_PAL, {
					player_id: appState.selectedPlayer!.uid,
					character_id,
					nickname
				});
			case 'gps':
				res = await sendAndWait(MessageType.ADD_GPS_PAL, {
					character_id,
					nickname
				});
				break;
		}

		if (!res) {
			let message = `Failed to add pal ${character_id}`;
			if (target === 'pal-box') {
				message += ` to pal box for player ${appState.selectedPlayer!.uid}`;
			} else if (target === 'dps') {
				message += ` to dps for player ${appState.selectedPlayer!.uid}`;
			} else if (target === 'gps') {
				message += ` to GPS`;
			}
			console.error(message);
			return;
		}
		res.pal.name = name;
		handleApplyPalPreset(res.pal);
		switch (target) {
			case 'pal-box':
				appState.selectedPlayer!.pals![res.pal.instance_id] = res.pal;
				break;
			case 'dps':
				if (appState.selectedPlayer!.dps) {
					appState.selectedPlayer!.dps[res.index] = res.pal;
				} else {
					appState.selectedPlayer!.dps = { [res.index]: res.pal };
				}
				break;
			case 'gps':
				if (appState.gps) {
					appState.gps[res.index] = res.pal;
				} else {
					appState.gps = { [res.index]: res.pal };
				}
				break;
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
				if (!canBeLucky(character_id)[1]) continue;
				const pal = await addPal(character_id, nickname, palData.localized_name || character_id);
				if (!pal) {
					console.error(`Failed to add lucky pal for ${character_id}`);
					continue;
				}
				pal.is_lucky = true;
				pal.state = EntryState.MODIFIED;
			}
			if (addAlphaPals) {
				if (!canBeAlpha(character_id)[1]) continue;
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

	async function createSpecialPalVariants(pals: [string, PalData][]) {
		for (const [character_id, palData] of pals) {
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
		if (addPredatorPals) await createSpecialPalVariants(predatorPals);
		if (addRaidPals) await createSpecialPalVariants(raidPals);
		if (addSummonPals) await createSpecialPalVariants(summonPals);
		if (addOilRigPals) await createSpecialPalVariants(oilRigPals);
		if (addHumanPals) await createSpecialPalVariants(humanPals);
		if (addBossPals) await createSpecialPalVariants(bossPals);
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
				<span class="text-surface-500 text-2xl">{m.created()}</span>
				<span class="text-4xl font-bold"><NumberFlow value={count} /></span>
				<span class="text-surface-500 text-2xl"> {m.of()} </span>
				<span class="text-surface-500 text-2xl">{totalRequiredSlots}</span>
				<span class="text-surface-500 text-2xl"> {c.pals.toLowerCase()} </span>
			</div>
		{:else}
			<div>
				<h6 class="h6">{m.type({ count: 2 })}</h6>
				<PalTypeToggles {toggles} />
			</div>
			<Card background="preset-filled-surface-200-800">
				<h6 class="h6">{m.apply_presets()}</h6>
				<p class="text-sm">
					{m.presets_precedence()}
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
								<span class="font-bold">{m.all()}</span>
							</div>
						{/snippet}
						{#snippet listItem(preset)}
							<div class="flex items-center space-x-2">
								<span>{preset.name}</span>
								{#if preset.pal_preset?.lock}
									{@const palIcon = assetLoader.loadMenuImage(
										preset.pal_preset.character_id as string
									)}
									<img
										src={palIcon}
										alt={preset.pal_preset.character_id}
										class="ml-2 h-8 w-8"
									/>
									<Lock class="ml-2 h-4 w-4 text-red-500" />
								{/if}
								{#if preset.pal_preset?.lock_element}
									{@const elementData = elementsData.getByKey(
										preset.pal_preset.element as string
									)}
									{@const elementIcon = assetLoader.loadImage(
										`${ASSET_DATA_PATH}/img/${elementData?.badge_icon}.webp`
									)}
									<img src={elementIcon} alt={elementData?.name} class="ml-2 h-6 w-6" />
								{/if}
							</div>
						{/snippet}
						{#snippet listItemActions(preset)}
							<Button variant="ghost" size="icon" onclick={() =>
									(selectedPresets = selectedPresets.filter((p) => p.id !== preset.id))}>
								<Trash size={16} />
							</Button>
						{/snippet}
						{#snippet listItemPopup(preset)}
							<div class="flex items-center space-x-2">
								<span>{preset.name}</span>
								{#if preset.pal_preset?.lock}
									<Lock class="ml-2 h-4 w-4 text-red-500" />
								{/if}
								{#if preset.pal_preset?.lock_element}
									{@const elementData = elementsData.getByKey(
										preset.pal_preset.element as string
									)}
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
			<span class="font-bold"
				>{m.ready_to_fill({ target, count: totalRequiredSlots, pals: c.pals })}</span
			>
		</div>
	{:else if !canAddPals && !isBusy}
		<div class="mt-1 flex w-full justify-end text-sm text-red-500">
			<span class="font-bold">
				{m.cannot_fill({
					target,
					count: Math.abs(availablePalBoxSlots - totalRequiredSlots)
				})}
			</span>
		</div>
	{/if}
	<div class="flex justify-end space-x-4">
		<Tooltip position="bottom" label={m.cancel()}>
			<Button variant="secondary" onclick={handleCancel}>
				<X size={20} />
				<span>{m.cancel()}</span>
			</Button>
		</Tooltip>
		<Tooltip position="bottom" label={m.confirm()}>
			<Button variant="primary" onclick={handleConfirm} disabled={!canAddPals || isBusy}>
				<Check size={20} />
				<span>{m.fill()}</span>
			</Button>
		</Tooltip>
	</div>
</Card>