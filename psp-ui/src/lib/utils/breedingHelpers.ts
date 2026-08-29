import { passiveSkillsData } from '$lib/data/passiveSkills.svelte';

export function palSkillName(asset: string): string {
	const skill = passiveSkillsData.getByKey(asset);
	return skill?.localized_name ?? asset;
}
