/**
 * Breeding display helpers — shared between page + components.
 */
import { passiveSkillsData } from '$lib/data/passiveSkills.svelte';

/** Resolve a passive skill ID ("CraftSpeed_up1") to its localized name ("Serious").
 *  Falls back to the raw id if the catalog isn't loaded or the key is unknown. */
export function palSkillName(asset: string): string {
	const skill = passiveSkillsData.getByKey(asset);
	return skill?.localized_name ?? asset;
}
