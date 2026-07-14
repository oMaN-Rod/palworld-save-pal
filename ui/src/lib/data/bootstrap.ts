import { send } from '$lib/utils/websocketUtils';
import { getStoredSessionId, markReattachPending } from '$lib/utils/sessionPersistence';
import { getUpsState } from '$states/upsState.svelte';
import { MessageType } from '$types';
import { activeSkillsData } from './activeSkills.svelte';
import { bosses } from './bosses.svelte';
import { buildingsData } from './buildings.svelte';
import { elementsData } from './elements.svelte';
import { expData } from './exp.svelte';
import { friendshipData } from './friendship.svelte';
import { itemsData } from './items.svelte';
import { labResearchData } from './labResearch.svelte';
import { fastTravelPoints } from './fastTravelPoints.svelte';
import { mapObjects } from './mapObjects.svelte';
import { missionsData } from './missions.svelte';
import { palsData } from './pals.svelte';
import { passiveSkillsData } from './passiveSkills.svelte';
import { presetsData } from './presets.svelte';
import { relicData } from './relic.svelte';
import { relics } from './relics.svelte';
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
	await fastTravelPoints.reset();
	await relics.reset();
	await bosses.reset();
	await labResearchData.reset();
	await missionsData.reset();
	await relicData.reset();
	const upsState = getUpsState();
	await upsState.loadAll();

	send(MessageType.GET_VERSION);

	// Frames are processed FIFO: send sync first so it runs pre-reattach (no
	// save yet, settings-only emit), then reattach emits the overview once.
	send(MessageType.SYNC_APP_STATE);

	// Reattach to the last session if this tab held one before the refresh.
	const storedSessionId = getStoredSessionId();
	if (storedSessionId) {
		markReattachPending();
		send(MessageType.REATTACH_SESSION, { session_id: storedSessionId });
	}
};
