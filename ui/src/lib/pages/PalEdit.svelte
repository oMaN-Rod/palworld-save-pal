<script lang="ts">
	import { assetLoader } from '$utils/asset-loader';
	import {
		ActiveSkillBadge,
		PassiveSkillBadge,
		StatsBadges,
		WorkSuitabilities,
		TextInputModal,
		Spinner
	} from '$components';
	import { Card, CornerDotButton, SectionHeader, Tooltip } from '$components/ui';
	import { type Pal, PalGender } from '$types';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { palsData, elementsData } from '$lib/data';
	import { cn } from '$theme';
	import { getAppState, getModalState } from '$states';

	const appState = getAppState();
	const modal = getModalState();

	let palLevel: string = $state('');
	let palLevelClass: string = $state('');

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

	// function handleLevelDecrement() {
	// 	if (!appState.selectedPal) return;
	// 	appState.selectedPal.level = Math.max(appState.selectedPal.level - 1, 1);
	// 	appState.selectedPlayer.pals[appState.selectedPalId].level = appState.selectedPal.level;
	// }

	// function handleLevelIncrement() {
	// 	if (!appState.selectedPal) return;
	// 	appState.selectedPal.level = Math.min(appState.selectedPal.level + 1, 99);
	// 	appState.selectedPlayer.pals[appState.selectedPalId].level = appState.selectedPal.level;
	// }

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

	async function getPalElementTypes(character_id: string): Promise<string[] | undefined> {
		const pal = await palsData.getPalInfo(character_id);
		if (!pal) return undefined;
		return pal.elements.length > 0 ? pal.elements.map((e) => e.toLowerCase()) : undefined;
	}

	async function getPalElementBadge(elementType: string): Promise<string | undefined> {
		const elementObj = await elementsData.searchElement(elementType);
		if (!elementObj) return undefined;
		const icon_path = `${ASSET_DATA_PATH}/img/elements/${elementObj.badge_icon}.png`;
		const icon = await assetLoader.loadImage(icon_path, true);
		return icon;
	}

	async function getGenderIcon(gender: PalGender): Promise<string | undefined> {
		if (gender == PalGender.UNKNOWN) return undefined;
		const iconPath = `${ASSET_DATA_PATH}/img/icons/${gender.toLowerCase()}.svg`;
		const icon = await assetLoader.loadSvg(iconPath);
		return icon;
	}

	async function handleEditNickname() {
		if (!appState.selectedPal) return;
		// @ts-ignore
		const result = await modal.showModal(TextInputModal, {
			title: 'Edit nickname',
			value: appState.selectedPal.nickname || appState.selectedPal.name
		});
		if (!result) return;
		appState.selectedPal.nickname = result as string;
		if (appState.selectedPlayer && appState.selectedPlayer.pals)
			appState.selectedPlayer.pals[appState.selectedPal.instance_id].nickname = result as string;
	}

	function handleUpdateActiveSkill(newSkill: string, oldSkill: string): void {
		if (appState.selectedPal) {
			const targetSkill = appState.selectedPal.active_skills.findIndex((s) => s === oldSkill);
			if (targetSkill >= 0) {
				appState.selectedPal.active_skills[targetSkill] = newSkill.replace('EPalWazaID::', '');
			} else {
				appState.selectedPal.active_skills.push(newSkill.replace('EPalWazaID::', ''));
			}
		}
	}

	function handleUpdatePassiveSkill(newSkill: string, oldSkill: string): void {
		if (appState.selectedPal) {
			const targetSkill = appState.selectedPal.passive_skills.findIndex((s) => s === oldSkill);
			if (targetSkill >= 0) {
				appState.selectedPal.passive_skills[targetSkill] = newSkill;
			} else {
				appState.selectedPal.passive_skills.push(newSkill);
			}
		}
	}

	function handleEditGender() {
		if (appState.selectedPal) {
			const currentGender = appState.selectedPal.gender;
			appState.selectedPal.gender =
				currentGender === PalGender.MALE ? PalGender.FEMALE : PalGender.MALE;
		}
	}

	function setBasePreset(preset: string) {
		if (appState.selectedPal) {
			switch (preset) {
				case 'Base':
					appState.selectedPal.passive_skills = [
						'CraftSpeed_up2',
						'PAL_Sanity_Down_2',
						'Rare',
						'PAL_FullStomach_Down_2'
					];
					break;
				case 'Worker':
					appState.selectedPal.passive_skills = [
						'CraftSpeed_up2',
						'CraftSpeed_up1',
						'Rare',
						'PAL_CorporateSlave'
					];
					break;
				case 'Runner':
					appState.selectedPal.passive_skills = [
						'MoveSpeed_up_3',
						'MoveSpeed_up_2',
						'MoveSpeed_up_1',
						'Legend'
					];
					break;
				case 'Tank':
					appState.selectedPal.passive_skills = [
						'Deffence_up2',
						'Deffence_up1',
						'PAL_masochist',
						'Legend'
					];
					break;
				case 'Attack':
					appState.selectedPal.passive_skills = ['Noukin', 'PAL_ALLAttack_up2', 'Rare', 'Legend'];
					break;
				case 'Balanced':
					appState.selectedPal.passive_skills = [
						'Noukin',
						'PAL_ALLAttack_up2',
						'Deffence_up2',
						'Legend'
					];
					break;
				case 'Mount':
					appState.selectedPal.passive_skills = [
						'Noukin',
						'PAL_ALLAttack_up2',
						'MoveSpeed_up_3',
						'Legend'
					];
					break;
				case 'Element':
					const palType = appState.selectedPal.elements[0];
					appState.selectedPal.passive_skills = [
						'Noukin',
						'PAL_ALLAttack_up2',
						'Legend',
						getElementPassive(palType)
					];
					break;
			}
		}
	}

	function getElementPassive(element: string): string {
		console.log('Element:', element);
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

	$effect(() => {
		if (appState.selectedPlayer && appState.selectedPal) {
			palLevel =
				appState.selectedPlayer.level < appState.selectedPal.level
					? appState.selectedPlayer.level.toString()
					: appState.selectedPal.level.toString();
			palLevelClass =
				appState.selectedPlayer.level < appState.selectedPal.level ? 'text-error-500' : '';
		}
	});
</script>

{#if appState.selectedPal}
	<div class="p-4">
		<div class="flex flex-row">
			<div
				class="card border-l-surface-600 preset-filled-surface-100-900 my-2 flex flex-row rounded-none border-l-2 p-4"
			>
				<!-- <button class="mr-4">
										<Minus class="text-primary-500" onclick={handleLevelDecrement} />
									</button> -->
				<div class="flex flex-col items-center justify-center">
					<span class={cn('text-surface-400 font-bold', palLevelClass)}>LEVEL</span>
					<span class={cn('text-4xl font-bold', palLevelClass)}>{palLevel}</span>
				</div>
				<!-- <button class="ml-4">
										<Plus class="text-primary-500" onclick={handleLevelIncrement} />
									</button> -->
			</div>

			<div class="grow">
				<Card class="my-2 ml-2 w-1/3 rounded-none p-4">
					<div class="flex flex-col">
						<div class="flex flex-row">
							<span class="grow">{appState.selectedPal.name}</span>
							<Tooltip position="bottom">
								<CornerDotButton label="Edit" onClick={handleEditNickname} />
								{#snippet popup()}
									<span>Edit nickname</span>
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
						</div>
						<hr class="hr my-1" />
						<div class="flex flex-row">
							<span class="text-surface-400 grow">{appState.selectedPal.nickname}</span>
							<div class="mt-2 flex flex-row">
								{#await getPalElementTypes(appState.selectedPal.character_id) then elementTypes}
									{#if elementTypes}
										{#each elementTypes as elementType}
											{#await getPalElementBadge(elementType) then icon}
												{#if icon}
													<enhanced:img
														src={icon}
														alt={elementType}
														class="pal-element-badge"
														style="width: 24px; height: 24px;"
													></enhanced:img>
												{/if}
											{/await}
										{/each}
									{/if}
								{/await}
							</div>
						</div>
					</div>
				</Card>
			</div>
		</div>
		<div class="grid w-full grid-cols-3 gap-2">
			<div class="flex flex-col space-y-2">
				<SectionHeader text="Active Skills" />
				{#each getActiveSkills(appState.selectedPal) as skill}
					<ActiveSkillBadge
						{skill}
						onSkillUpdate={handleUpdateActiveSkill}
						palCharacterId={appState.selectedPal.character_id}
					/>
				{/each}
				<SectionHeader text="Passive Skills" />
				<div class="grid grid-cols-2 gap-2">
					{#each getPassiveSkills(appState.selectedPal) as skill}
						<PassiveSkillBadge {skill} onSkillUpdate={handleUpdatePassiveSkill} />
					{/each}
				</div>
				<SectionHeader text="Utility Presets" />
				<div class="btn-group preset-outlined-surface-100-900 my-2 flex-col p-2 md:flex-row">
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Base')}>Base</button
					>
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Worker')}>Worker</button
					>
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Runner')}>Runner</button
					>
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Tank')}>Tank</button
					>
				</div>
				<SectionHeader text="Attack Presets" />
				<div class="btn-group preset-outlined-surface-100-900 my-2 flex-col p-2 md:flex-row">
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Attack')}>Attack</button
					>
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Balanced')}>Balanced</button
					>
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Mount')}>Mount</button
					>
					<button
						type="button"
						class="btn hover:bg-primary-500"
						onclick={() => setBasePreset('Element')}>Element</button
					>
				</div>
			</div>
			<div class="flex flex-col items-center justify-center">
				{#await loadPalImage() then palImage}
					{#if palImage}
						<div class="pal">
							<enhanced:img src={palImage} alt={`${appState.selectedPal.name} icon`}></enhanced:img>
						</div>
					{:else}
						<div class="flex h-96 w-96 items-center justify-center">
							<Spinner size="size-48" />
						</div>
					{/if}
				{/await}
			</div>
			<div class="flex flex-col space-y-2">
				<SectionHeader text="Stats" />
				<StatsBadges bind:pal={appState.selectedPal} bind:player={appState.selectedPlayer} />
				<SectionHeader text="Work Suitability" />
				<WorkSuitabilities bind:pal={appState.selectedPal} />
			</div>
		</div>
	</div>
{:else}
	<div class="flex w-full items-center justify-center">
		<h2 class="h2">Upload a save file and select a Pal to edit 🚀</h2>
	</div>
{/if}

<style lang="postcss">
	.pal img {
		max-height: 600px;
	}
</style>
