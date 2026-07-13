<script lang="ts">
	import { ActiveSkillOption, PassiveSkillOption, Talents } from '$components/pal';
	import { Button, Combobox, CornerDotButton, List } from '$components/ui';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { activeSkillsData, passiveSkillsData } from '$lib/data';
	import { isSkillAvailableForCharacter } from '$lib/utils/skillFilters';
	import { getAppState } from '$states';
	import { PalGender, type EggConfig, type Pal, type SelectOption } from '$types';
	import { assetLoader } from '$utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import { TimerReset, Trash } from 'lucide-svelte';
	import { getPalIcon } from './itemUtils';

	let {
		eggConfig = $bindable(),
		palOptions
	}: {
		eggConfig: EggConfig;
		palOptions: SelectOption[];
	} = $props();

	const appState = getAppState();

	let accordionValue = $state<string[]>(['pal']);

	const activeSkillOptions: SelectOption[] = $derived(
		Object.values(activeSkillsData.activeSkills)
			.filter((skill) => isSkillAvailableForCharacter(skill.id, eggConfig.character_id))
			.filter(
				(aSkill) => !Object.values(eggConfig.active_skills).some((skill) => skill === aSkill.id)
			)
			.sort((a, b) => a.details.element.localeCompare(b.details.element))
			.map((s) => ({
				value: s.id,
				label: s.localized_name
			}))
	);
	const learnedSkillsOptions: SelectOption[] = $derived(
		Object.values(activeSkillsData.activeSkills)
			.filter((skill) => isSkillAvailableForCharacter(skill.id, eggConfig.character_id))
			.filter(
				(aSkill) =>
					!Object.values(eggConfig.learned_skills).some((skill) => skill === aSkill.id)
			)
			.sort((a, b) => a.details.element.localeCompare(b.details.element))
			.map((s) => ({
				value: s.id,
				label: s.localized_name
			}))
	);
	const passiveSkillOptions: SelectOption[] = $derived(
		Object.values(passiveSkillsData.passiveSkills)
			.filter((pSkill) => !pSkill.details.disabled)
			.filter(
				(pSkill) => !Object.values(eggConfig.passive_skills).some((p) => p === pSkill.id)
			)
			.sort((a, b) => b.details.rank - a.details.rank)
			.map((s) => ({
				value: s.id,
				label: s.localized_name
			}))
	);
	const activeSkillAddDisabled = $derived(
		!appState.settings.cheat_mode && eggConfig.active_skills.length >= 3
	);
	const passiveSkillAddDisabled = $derived(
		!appState.settings.cheat_mode && eggConfig.passive_skills.length >= 4
	);
</script>

<Accordion
	classes="w-full"
	value={accordionValue}
	onValueChange={(e: ValueChangeDetails) => (accordionValue = e.value)}
	collapsible
>
	<Accordion.Item
		controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
		value="pal"
		controlHover="hover:bg-secondary-500/25"
	>
		{#snippet control()}
			{c.pal}
		{/snippet}
		{#snippet panel()}
			<div class="flex w-full items-center space-x-2">
				<Combobox
					options={palOptions}
					bind:value={eggConfig.character_id}
					placeholder={m.select_entity({ entity: c.pal })}
				>
					{#snippet selectOption(option)}
						<div class="flex items-center space-x-2">
							<img
								src={getPalIcon(option.value as string)}
								alt={option.label}
								class="h-8 w-8"
							/>
							<span>{option.label}</span>
						</div>
					{/snippet}
				</Combobox>
				<CornerDotButton
					onClick={() => {
						eggConfig.gender =
							eggConfig.gender === PalGender.FEMALE ? PalGender.MALE : PalGender.FEMALE;
					}}
					class="h-8 w-8 p-1"
				>
					<img
						src={assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${eggConfig.gender}.webp`)}
						alt={eggConfig.gender}
					/>
				</CornerDotButton>
			</div>
		{/snippet}
	</Accordion.Item>
	<Accordion.Item
		controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
		value="active_skills"
		controlHover="hover:bg-secondary-500/25"
	>
		{#snippet control()}
			{m.active_skill({ count: 2 })}
		{/snippet}
		{#snippet panel()}
			<Combobox
				options={activeSkillOptions}
				placeholder={m.select_entity({ entity: m.active_skill({ count: 1 }) })}
				onChange={(value) => {
					eggConfig.active_skills.push(value as string);
				}}
				disabled={activeSkillAddDisabled}
			>
				{#snippet selectOption(option)}
					<ActiveSkillOption {option} />
				{/snippet}
			</Combobox>

			{#if eggConfig.active_skills.length > 0}
				<List
					items={eggConfig.active_skills}
					listClass="max-h-60 overflow-y-auto"
					canSelect={false}
					multiple={false}
				>
					{#snippet listHeader()}
						<div>
							<span class="font-bold">{m.active_skill({ count: 2 })}</span>
						</div>
					{/snippet}
					{#snippet listItem(skill: string)}
						{@const activeSkill = activeSkillsData.getByKey(skill)}
						<ActiveSkillOption
							option={{ label: activeSkill?.localized_name || skill, value: skill }}
						/>
					{/snippet}
					{#snippet listItemActions(skill: string)}
						<Button
							variant="ghost" size="icon"
							onclick={() =>
								(eggConfig.active_skills = eggConfig.active_skills.filter(
									(s) => s !== skill
								))}
						>
							<Trash size={16} />
						</Button>
					{/snippet}
					{#snippet listItemPopup(skill: string)}
						{@const activeSkill = activeSkillsData.getByKey(skill)}
						<div class="flex items-center space-x-1 justify-self-start">
							<TimerReset class="h-4 w-4" />
							<span class="font-bold">{activeSkill?.details.cool_time}</span>
							<span class="text-xs">{m.pwr()}</span>
							<span class="font-bold">{activeSkill?.details.power}</span>
						</div>
					{/snippet}
				</List>
			{/if}
		{/snippet}
	</Accordion.Item>
	<Accordion.Item
		controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
		value="learned_skills"
		controlHover="hover:bg-secondary-500/25"
	>
		{#snippet control()}
			{m.learned_skills()}
		{/snippet}
		{#snippet panel()}
			<Combobox
				options={learnedSkillsOptions}
				placeholder={m.select_entity({ entity: m.active_skill({ count: 2 }) })}
				onChange={(value) => {
					eggConfig.learned_skills.push(value as string);
				}}
			>
				{#snippet selectOption(option)}
					<ActiveSkillOption {option} />
				{/snippet}
			</Combobox>

			{#if eggConfig.learned_skills.length > 0}
				<List
					items={eggConfig.learned_skills}
					listClass="max-h-60 overflow-y-auto"
					canSelect={false}
					multiple={false}
				>
					{#snippet listHeader()}
						<div>
							<span class="font-bold">{m.learned_skills()}</span>
						</div>
					{/snippet}
					{#snippet listItem(skill: string)}
						{@const activeSkill = activeSkillsData.getByKey(skill)}
						<ActiveSkillOption
							option={{ label: activeSkill?.localized_name || skill, value: skill }}
						/>
					{/snippet}
					{#snippet listItemActions(skill: string)}
						<Button
							variant="ghost" size="icon"
							onclick={() =>
								(eggConfig.learned_skills = eggConfig.learned_skills.filter(
									(s) => s !== skill
								))}
						>
							<Trash size={16} />
						</Button>
					{/snippet}
					{#snippet listItemPopup(skill: string)}
						{@const activeSkill = activeSkillsData.getByKey(skill)}
						<div class="flex items-center space-x-1 justify-self-start">
							<TimerReset class="h-4 w-4" />
							<span class="font-bold">{activeSkill?.details.cool_time}</span>
							<span class="text-xs">{m.pwr()}</span>
							<span class="font-bold">{activeSkill?.details.power}</span>
						</div>
					{/snippet}
				</List>
			{/if}
		{/snippet}
	</Accordion.Item>
	<Accordion.Item
		controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
		value="passive_skills"
		controlHover="hover:bg-secondary-500/25"
	>
		{#snippet control()}
			{m.passive_skill({ count: 2 })}
		{/snippet}
		{#snippet panel()}
			<Combobox
				label={m.passive_skill({ count: 2 })}
				options={passiveSkillOptions}
				placeholder={m.select_entity({ entity: m.passive_skill({ count: 2 }) })}
				onChange={(value) => eggConfig.passive_skills.push(value as string)}
				disabled={passiveSkillAddDisabled}
			>
				{#snippet selectOption(option)}
					<PassiveSkillOption {option} />
				{/snippet}
			</Combobox>

			{#if eggConfig.passive_skills.length > 0}
				<List
					items={eggConfig.passive_skills}
					listClass="max-h-60 overflow-y-auto"
					canSelect={false}
					multiple={false}
				>
					{#snippet listHeader()}
						<div>
							<span class="font-bold">{m.passive_skill({ count: 2 })}</span>
						</div>
					{/snippet}
					{#snippet listItem(skill: string)}
						{@const passiveSkill = passiveSkillsData.getByKey(skill)}
						<PassiveSkillOption
							option={{ label: passiveSkill?.localized_name || skill, value: skill }}
						/>
					{/snippet}
					{#snippet listItemActions(skill: string)}
						<Button
							variant="ghost" size="icon"
							onclick={() =>
								(eggConfig.passive_skills = eggConfig.passive_skills.filter(
									(s) => s !== skill
								))}
						>
							<Trash size={16} />
						</Button>
					{/snippet}
					{#snippet listItemPopup(skill: string)}
						{@const passiveSkill = passiveSkillsData.getByKey(skill)}
						<div class="flex grow flex-col">
							<span class="grow truncate">{passiveSkill?.localized_name || skill}</span>
							<span class="text-xs">{passiveSkill?.description}</span>
						</div>
					{/snippet}
				</List>
			{/if}
		{/snippet}
	</Accordion.Item>
	<Accordion.Item
		controlBase="flex text-start items-center space-x-4 w-full bg-surface-800"
		value="talents"
		controlHover="hover:bg-secondary-500/25"
	>
		{#snippet control()}
			{m.talents_ivs()}
		{/snippet}
		{#snippet panel()}
			<Talents bind:pal={eggConfig as Pal} />
		{/snippet}
	</Accordion.Item>
</Accordion>