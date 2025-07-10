<script lang="ts">
	import { Card, Tooltip, Combobox } from '$components/ui';
	import { type Pal, type SelectOption, type SkillType } from '$types';
	import { Save, X, Delete } from 'lucide-svelte';
	import { activeSkillsData, passiveSkillsData } from '$lib/data';
	import { ActiveSkillOption, PassiveSkillOption } from '$components';

	let {
		title = '',
		value = $bindable(''),
		type = 'Active',
		pal,
		closeModal
	} = $props<{
		title?: string;
		value?: string;
		type?: SkillType;
		pal?: Pal;
		closeModal: (value: any) => void;
	}>();

	let selectOptions: SelectOption[] = $derived.by(() => {
		if (type === 'Active') {
			return Object.values(activeSkillsData.activeSkills)
				.filter((skill) => {
					if (skill.id.toLowerCase().includes(`unique_${pal.character_key.toLowerCase()}`)) {
						return true;
					}
					if (!skill.id.toLowerCase().includes('unique_')) {
						return true;
					}
					return false;
				})
				.filter((aSkill) => !Object.values(pal.active_skills).some((skill) => skill === aSkill.id))
				.sort((a, b) => a.details.element.localeCompare(b.details.element))
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		} else {
			return Object.values(passiveSkillsData.passiveSkills)
				.filter((pSkill) => !Object.values(pal.passive_skills).some((p) => p === pSkill.id))
				.sort((a, b) => b.details.rank - a.details.rank)
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		}
	});

	function handleClear() {
		closeModal('Empty');
	}

	function handleClose(value: any) {
		closeModal(value);
	}
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<Combobox options={selectOptions} bind:value>
		{#snippet selectOption(option)}
			{#if type === 'Active'}
				<ActiveSkillOption {option} />
			{:else}
				<PassiveSkillOption {option} />
			{/if}
		{/snippet}
	</Combobox>

	<div class="mt-2 flex flex-row items-center space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={handleClear}>
				<Delete />
			</button>
			{#snippet popup()}
				<span>Clear</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(value)}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(null)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
