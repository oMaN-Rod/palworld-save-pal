<script lang="ts">
	import { assetLoader } from '$utils/asset-loader';
	import {
		ActiveSkillBadge,
		PassiveSkillBadge,
		StatsBadges,
		WorkSuitabilities,
		TextInputModal,
		Talents,
		LearnedSkillSelectModal
	} from '$components';
	import { CornerDotButton, Progress, SectionHeader, Tooltip } from '$components/ui';
	import { type ElementType, EntryState, type Pal, PalGender, type PresetProfile } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { palsData, elementsData, expData, presetsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getModalState } from '$states';
	import { Rating } from '@skeletonlabs/skeleton-svelte';
	import { Minus, Plus, Brain, Save } from 'lucide-svelte';
	import { Souls } from '$components';
	import { getStats } from '$lib/data';
	import SkillPresets from './SkillPresets.svelte';

	const appState = getAppState();
	const modal = getModalState();

	let palLevel: string = $state('');
	let palLevelClass: string = $state('');
	let palLevelMessage: string = $state('');
	let palLevelProgressToNext: number = $state(0);
	let palLevelProgressValue: number = $state(0);
	let palLevelProgressMax: number = $state(1);

	let alphaIcon: string = $state('');

	async function loadPalImage(): Promise<string | undefined> {
		const pal = $state.snapshot(appState.selectedPal);
		if (pal) {
			const { name } = pal;
			let imagePath = `${ASSET_DATA_PATH}/img/pals/full/${name.toLowerCase().replaceAll(' ', '_')}.png`;
			const image = await assetLoader.loadImage(imagePath, true);
			return image;
		}
		return undefined;
	}

	async function loadStaticIcons() {
		const iconPath = `${ASSET_DATA_PATH}/img/icons/Alpha.png`;
		const icon = await assetLoader.loadImage(iconPath);
		alphaIcon = icon;
	}

	async function initPalLevelProgress() {
		if (appState.selectedPal) {
			if (appState.selectedPal.level === 55) {
				palLevelProgressToNext = 0;
				palLevelProgressValue = 0;
				palLevelProgressMax = 1;
				return;
			}
			const nextExp = await expData.getExpDataByLevel(appState.selectedPal.level + 1);
			palLevelProgressToNext = nextExp.PalTotalEXP - appState.selectedPal.exp;
			palLevelProgressValue = nextExp.PalNextEXP - palLevelProgressToNext;
			palLevelProgressMax = nextExp.PalNextEXP;
		}
	}

	async function handleLevelIncrement() {
		if (!appState.selectedPal || !appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const newLevel = Math.min(appState.selectedPal.level + 1, 55);
		if (newLevel === appState.selectedPal.level) return;

		const nextLevelData = await expData.getExpDataByLevel(newLevel + 1);

		appState.selectedPal.level = newLevel;
		appState.selectedPal.exp = nextLevelData.PalTotalEXP - nextLevelData.PalNextEXP;
		appState.selectedPal.state = EntryState.MODIFIED;

		await initPalLevelProgress();
	}

	async function handleLevelDecrement() {
		if (!appState.selectedPal || !appState.selectedPlayer || !appState.selectedPlayer.pals) return;

		const newLevel = Math.max(appState.selectedPal.level - 1, 1);
		if (newLevel === appState.selectedPal.level) return;

		const newLevelData = await expData.getExpDataByLevel(newLevel + 1);

		appState.selectedPal.level = newLevel;
		appState.selectedPal.exp = newLevelData.PalTotalEXP - newLevelData.PalNextEXP;
		appState.selectedPal.state = EntryState.MODIFIED;

		await initPalLevelProgress();
	}

	function getActiveSkills(pal: Pal): string[] {
		let skills = [...pal.active_skills];
		while (skills.length < 3) {
			skills.push('Empty');
		}
		return skills;
	}

	function getPassiveSkills(pal: Pal): string[] {
		let skills = [...pal.passive_skills];
		while (skills.length < 4) {
			skills.push('Empty');
		}
		return skills;
	}

	async function getPalElementTypes(character_id: string): Promise<ElementType[] | undefined> {
		const palData = await palsData.getPalInfo(character_id);
		if (!palData) return undefined;
		return palData.element_types.length > 0 ? palData.element_types : undefined;
	}

	async function getPalDescription(character_id: string): Promise<string | undefined> {
		const palData = await palsData.getPalInfo(character_id);
		if (!palData) return undefined;
		return palData.description;
	}

	async function getPalElementBadge(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		console.log('element', elementType, elementObj);
		if (!elementObj) return undefined;
		const icon_path = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`;
		const icon = await assetLoader.loadImage(icon_path, true);
		return icon;
	}

	async function getGenderIcon(gender: PalGender): Promise<string | undefined> {
		const iconPath = `${ASSET_DATA_PATH}/img/icons/${gender.toLowerCase()}.svg`;
		const icon = await assetLoader.loadSvg(iconPath);
		return icon;
	}

	async function handleEditNickname() {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit nickname',
			value: appState.selectedPal.nickname || appState.selectedPal.name
		});
		if (!result) return;
		appState.selectedPal.nickname = result;
		appState.selectedPal.state = EntryState.MODIFIED;
		if (appState.selectedPlayer && appState.selectedPlayer.pals)
			appState.selectedPlayer.pals[appState.selectedPal.instance_id].nickname = result;
	}

	async function handleEditLearnedSkills() {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string[]>(LearnedSkillSelectModal, {
			pal: appState.selectedPal
		});
		if (result) {
			appState.selectedPal.learned_skills = result;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	async function handleMaxOutPal() {
		if (!appState.selectedPal || !appState.selectedPlayer) return;
		appState.selectedPal.level = 55;
		appState.selectedPal.is_boss = true;
		appState.selectedPal.is_lucky = false;
		appState.selectedPal.talent_hp = 100;
		appState.selectedPal.talent_shot = 100;
		appState.selectedPal.talent_defense = 100;
		appState.selectedPal.rank = 4;
		appState.selectedPal.rank_hp = 10;
		appState.selectedPal.rank_defense = 10;
		appState.selectedPal.rank_attack = 10;
		appState.selectedPal.rank_craftspeed = 10;
		await getStats(appState.selectedPal, appState.selectedPlayer);
		appState.selectedPal.hp = appState.selectedPal.max_hp;
		appState.selectedPal.state = EntryState.MODIFIED;
		const palData = await palsData.getPalInfo(appState.selectedPal.character_id);
		if (palData) {
			appState.selectedPal.stomach = palData.max_full_stomach;
			const palType = palData.element_types[0];
			appState.selectedPal.passive_skills = [
				'Noukin',
				'PAL_ALLAttack_up2',
				'Legend',
				getElementPassive(palType)
			];
		} else {
			appState.selectedPal.stomach = 150;
		}
	}

	function handleUpdateActiveSkill(newSkill: string, oldSkill: string): void {
		if (appState.selectedPal) {
			const targetSkillIndex = appState.selectedPal.active_skills.findIndex((s) => s === oldSkill);

			if (newSkill === 'Empty') {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.active_skills.splice(targetSkillIndex, 1);
				}
			} else {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.active_skills[targetSkillIndex] = newSkill;
				} else {
					appState.selectedPal.active_skills.push(newSkill);
				}
			}

			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleUpdatePassiveSkill(newSkill: string, oldSkill: string): void {
		if (appState.selectedPal) {
			const targetSkillIndex = appState.selectedPal.passive_skills.findIndex((s) => s === oldSkill);

			if (newSkill === 'Empty') {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.passive_skills.splice(targetSkillIndex, 1);
				}
			} else {
				if (targetSkillIndex >= 0) {
					appState.selectedPal.passive_skills[targetSkillIndex] = newSkill;
				} else {
					appState.selectedPal.passive_skills.push(newSkill);
				}
			}

			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleEditGender() {
		if (appState.selectedPal) {
			const currentGender = appState.selectedPal.gender;
			appState.selectedPal.gender =
				currentGender === PalGender.MALE ? PalGender.FEMALE : PalGender.MALE;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	async function setSkillPreset(type: 'active' | 'passive', skills: string[]) {
		if (appState.selectedPal) {
			if (type === 'active') {
				appState.selectedPal.active_skills = skills || [];
			} else {
				appState.selectedPal.passive_skills = skills || [];
			}
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function getElementPassive(element: string): string {
		switch (element.toLowerCase()) {
			case 'neutral':
				return 'ElementBoost_Normal_2_PAL';
			case 'dark':
				return 'ElementBoost_Dark_2_PAL';
			case 'dragon':
				return 'ElementBoost_Dragon_2_PAL';
			case 'ice':
				return 'ElementBoost_Ice_2_PAL';
			case 'fire':
				return 'ElementBoost_Fire_2_PAL';
			case 'grass':
				return 'ElementBoost_Leaf_2_PAL';
			case 'ground':
				return 'ElementBoost_Earth_2_PAL';
			case 'electric':
				return 'ElementBoost_Thunder_2_PAL';
			case 'water':
				return 'ElementBoost_Aqua_2_PAL';
			default:
				return 'Rare';
		}
	}

	function handleEditLucky() {
		if (appState.selectedPal) {
			appState.selectedPal.is_lucky = !appState.selectedPal.is_lucky;
			appState.selectedPal.is_boss = appState.selectedPal.is_lucky
				? false
				: appState.selectedPal.is_boss;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}

	function handleEditAlpha() {
		if (appState.selectedPal) {
			appState.selectedPal.is_boss = !appState.selectedPal.is_boss;
			appState.selectedPal.is_lucky = appState.selectedPal.is_boss
				? false
				: appState.selectedPal.is_lucky;
			appState.selectedPal.state = EntryState.MODIFIED;
		}
	}
	async function handleAddPreset(type: 'active' | 'passive') {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: `Add ${type} skills preset`,
			value: '',
			inputLabel: 'Preset name'
		});
		if (!result) return;
		const skills =
			type === 'active' ? appState.selectedPal.active_skills : appState.selectedPal.passive_skills;
		const newPreset = {
			name: result,
			type: type === 'active' ? 'active_skills' : 'passive_skills',
			skills
		} as PresetProfile;

		await presetsData.addPresetProfile(newPreset);
	}

	$effect(() => {
		if (appState.selectedPlayer && appState.selectedPal) {
			palLevel =
				appState.selectedPlayer.level < appState.selectedPal.level
					? appState.selectedPlayer.level.toString()
					: appState.selectedPal.level.toString();
			palLevelClass =
				appState.selectedPlayer.level < appState.selectedPal.level ? 'text-error-500' : '';
			palLevelMessage =
				appState.selectedPlayer.level < appState.selectedPal.level ? 'Level sync' : 'No Level Sync';
		}
	});

	$effect(() => {
		loadStaticIcons();
		initPalLevelProgress();
	});
</script>

{#if appState.selectedPal}
	<div class="flex h-full overflow-auto p-2">
		<div class="flex flex-grow flex-col">
			<div class="flex-shrink-0">
				<div
					class="border-l-surface-600 preset-filled-surface-100-900 flex w-3/4 flex-row rounded-none border-l-2 p-4"
				>
					<div class="mr-4 flex flex-col items-center justify-center rounded-none">
						<Rating bind:value={appState.selectedPal.rank} count={4} itemClasses="text-gray" />
						<div class="flex flex-row px-2">
							<button class="mr-4">
								<Minus class="text-primary-500" onclick={handleLevelDecrement} />
							</button>

							<Tooltip>
								<div class="flex flex-col items-center justify-center">
									<span class={cn('text-surface-400 font-bold', palLevelClass)}>LEVEL</span>
									<span class={cn('text-4xl font-bold', palLevelClass)}>{palLevel}</span>
								</div>
								{#snippet popup()}
									{palLevelMessage}
								{/snippet}
							</Tooltip>

							<button class="ml-4">
								<Plus class="text-primary-500" onclick={handleLevelIncrement} />
							</button>
						</div>
					</div>

					<div class="grow">
						<div class="flex flex-col">
							<div class="flex flex-row items-center space-x-2">
								<span class="grow">
									{appState.selectedPal.nickname || appState.selectedPal.name}
								</span>
								<Tooltip position="bottom">
									<CornerDotButton label="Edit" onClick={handleEditNickname} />
									{#snippet popup()}
										<span>Edit nickname</span>
									{/snippet}
								</Tooltip>
								<Tooltip position="bottom">
									<CornerDotButton label="Max" onClick={handleMaxOutPal} />
									{#snippet popup()}
										<span>Max out Pal stats 💉💪</span>
									{/snippet}
								</Tooltip>
								<Tooltip position="bottom">
									<button
										class="hover:ring-secondary-500 relative flex h-full w-auto items-center justify-center hover:ring"
										onclick={handleEditGender}
									>
										{#await getGenderIcon(appState.selectedPal.gender) then icon}
											{#if icon}
												{@const color =
													appState.selectedPal.gender == PalGender.MALE
														? 'text-primary-300'
														: 'text-tertiary-300'}
												<div class={cn('h-8 w-8', color)}>
													{@html icon}
												</div>
											{/if}
										{/await}
										<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
									</button>
									{#snippet popup()}
										<span>Toggle gender</span>
									{/snippet}
								</Tooltip>
								<Tooltip position="bottom">
									<button
										class={cn(
											'hover:ring-secondary-500 relative flex h-full w-auto items-center justify-center hover:ring',
											appState.selectedPal.is_lucky && 'bg-secondary-500/25'
										)}
										onclick={handleEditLucky}
									>
										<div class="flex h-8 w-8 items-center justify-center">✨</div>
										<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
									</button>
									{#snippet popup()}
										<span>Toggle Lucky</span>
									{/snippet}
								</Tooltip>
								<Tooltip position="bottom">
									<button
										class={cn(
											'hover:ring-secondary-500 relative flex h-full w-auto items-center justify-center hover:ring',
											appState.selectedPal.is_boss && 'bg-secondary-500/25'
										)}
										onclick={handleEditAlpha}
									>
										<div class="flex h-8 w-8 items-center justify-center">
											{#if alphaIcon}
												<enhanced:img
													src={alphaIcon}
													alt="Alpha"
													class="h-8 w-8"
													style="width: 24px; height: 24px;"
												></enhanced:img>
											{/if}
										</div>
										<span class="bg-surface-600 absolute left-0 top-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute right-0 top-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute bottom-0 left-0 h-0.5 w-0.5"></span>
										<span class="bg-surface-600 absolute bottom-0 right-0 h-0.5 w-0.5"></span>
									</button>
									{#snippet popup()}
										<span>Toggle Alpha</span>
									{/snippet}
								</Tooltip>
								<div class="flex flex-row items-center">
									<div class="flex flex-row">
										{#await getPalElementTypes(appState.selectedPal.character_id) then elementTypes}
											{#if elementTypes}
												{#each elementTypes as elementType}
													{#await getPalElementBadge(elementType) then icon}
														{#if icon}
															<enhanced:img src={icon} alt={elementType} class="h-8 w-8"
															></enhanced:img>
														{/if}
													{/await}
												{/each}
											{/if}
										{/await}
									</div>
								</div>
							</div>
							<hr class="hr my-1" />
							<div class="flex flex-col space-y-2">
								<div class="flex">
									<span class="text-on-surface grow">NEXT</span>
									<span class="text-on-surface">{palLevelProgressToNext}</span>
								</div>
								<Progress
									bind:value={palLevelProgressValue}
									bind:max={palLevelProgressMax}
									height="h-2"
									width="w-full"
									rounded="rounded-none"
									showLabel={false}
								/>
							</div>
						</div>
					</div>
				</div>
			</div>
			<div class="flex flex-grow">
				<div class="flex-1 overflow-auto p-2">
					<div class="flex flex-col space-y-2">
						<SectionHeader text="Active Skills">
							{#snippet action()}
								<div class="flex">
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={handleEditLearnedSkills}
										>
											<Brain size={20} />
										</button>
										{#snippet popup()}
											<span>Edit Learned Skills</span>
										{/snippet}
									</Tooltip>
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={() => handleAddPreset('active')}
										>
											<Save size={20} />
										</button>
										{#snippet popup()}
											<span>Save as preset</span>
										{/snippet}
									</Tooltip>
								</div>
							{/snippet}
						</SectionHeader>
						{#each getActiveSkills(appState.selectedPal) as skill}
							<ActiveSkillBadge
								{skill}
								onSkillUpdate={handleUpdateActiveSkill}
								palCharacterId={appState.selectedPal.character_id}
							/>
						{/each}
						<SectionHeader text="Passive Skills">
							{#snippet action()}
								<div class="flex">
									<Tooltip>
										<button
											class="btn hover:bg-secondary-500/25 ml-2 p-2"
											onclick={() => handleAddPreset('passive')}
										>
											<Save size={20} />
										</button>
										{#snippet popup()}
											<span>Save as preset</span>
										{/snippet}
									</Tooltip>
								</div>
							{/snippet}
						</SectionHeader>
						<div class="grid grid-cols-2 gap-2">
							{#each getPassiveSkills(appState.selectedPal) as skill}
								<PassiveSkillBadge {skill} onSkillUpdate={handleUpdatePassiveSkill} />
							{/each}
						</div>
						<SectionHeader text="Presets" />
						<SkillPresets onSelect={setSkillPreset} />
						<SectionHeader text="Work Suitability" />
						<WorkSuitabilities bind:pal={appState.selectedPal} />
					</div>
				</div>
				<div class="flex-1 overflow-auto p-2">
					<div class="flex h-full flex-col items-center justify-center">
						{#await loadPalImage() then palImage}
							{#if palImage}
								<div class="pal">
									<Tooltip
										popupClass="p-4 bg-surface-800"
										rounded="rounded-none"
										position="top-start"
										useArrow={false}
									>
										<enhanced:img
											src={palImage}
											alt={`${appState.selectedPal?.name} icon`}
											class="h-auto max-w-full"
										></enhanced:img>

										{#snippet popup()}
											{#await getPalDescription(appState.selectedPal!.character_id) then description}
												{#if description}
													<div class="flex max-w-96 flex-col">
														<p class="text-center">{description}</p>
													</div>
												{/if}
											{/await}
										{/snippet}
									</Tooltip>
								</div>
							{/if}
						{/await}
					</div>
				</div>
			</div>
		</div>
		<div class="w-1/3 overflow-auto p-2">
			<div class="flex flex-col space-y-2">
				<StatsBadges bind:pal={appState.selectedPal} bind:player={appState.selectedPlayer} />
				<SectionHeader text="Talents (IVs)" />
				<Talents bind:pal={appState.selectedPal} />
				<SectionHeader text="Souls" />
				<Souls bind:pal={appState.selectedPal} />
			</div>
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Upload a save file and select a Pal to edit 🚀</h2>
	</div>
{/if}
