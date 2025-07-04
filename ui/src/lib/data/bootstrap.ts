import { send } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';
import { activeSkillsData } from './activeSkills.svelte';
import { buildingsData } from './buildings.svelte';
import { elementsData } from './elements.svelte';
import { expData } from './exp.svelte';
import { friendshipData } from './friendship.svelte';
import { itemsData } from './items.svelte';
import { labResearchData } from './labResearch.svelte';
import { mapObjects } from './mapObjects.svelte';
import { palsData } from './pals.svelte';
import { passiveSkillsData } from './passiveSkills.svelte';
import { presetsData } from './presets.svelte';
import { technologiesData } from './technologies.svelte';
import { workSuitabilityData } from './workSuitability.svelte';

export const bootstrap = async () => {
	await presetsData.reset();
	await palsData.reset();
	await activeSkillsData.reset();
	await passiveSkillsData.reset();
	await technologiesData.reset();
	await elementsData.reset();
	await expData.reset();
	await friendshipData.reset();
	await itemsData.reset();
	await workSuitabilityData.reset();
	await buildingsData.reset();
	await mapObjects.reset();
	await labResearchData.reset();

	send(MessageType.GET_VERSION);
	send(MessageType.SYNC_APP_STATE);
};
