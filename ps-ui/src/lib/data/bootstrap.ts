import { send } from '$lib/utils/websocketUtils';
import { getStoredSessionId, markReattachPending } from '$lib/utils/sessionPersistence';
import { getUpsState } from '$states/upsState.svelte';
import { MessageType } from '$types';
import { activeSkillsData } from './activeSkills.svelte';
import { bosses } from './bosses.svelte';
import { buildingsData } from './buildings.svelte';
import { dungeons } from './dungeons.svelte';
import { elementsData } from './elements.svelte';
import { expData } from './exp.svelte';
import { friendshipData } from './friendship.svelte';
import { itemsData } from './items.svelte';
import { labResearchData } from './labResearch.svelte';
import { fastTravelPoints } from './fastTravelPoints.svelte';
import { missionsData } from './missions.svelte';
import { palsData } from './pals.svelte';
import { passiveSkillsData } from './passiveSkills.svelte';
import { presetsData } from './presets.svelte';
import { relicData } from './relic.svelte';
import { relics } from './relics.svelte';
import { technologiesData } from './technologies.svelte';
import { workSuitabilityData } from './workSuitability.svelte';

export const bootstrap = async () => {
	// Each reset requests a distinct game-data response type. Start them
	// together so the first screen is not delayed by 18 sequential localhost
	// round trips; the websocket queue still resolves each response by type.
	await Promise.all([
		presetsData.reset(),
		palsData.reset(),
		activeSkillsData.reset(),
		passiveSkillsData.reset(),
		technologiesData.reset(),
		elementsData.reset(),
		expData.reset(),
		friendshipData.reset(),
		itemsData.reset(),
		workSuitabilityData.reset(),
		buildingsData.reset(),
		dungeons.reset(),
		fastTravelPoints.reset(),
		relics.reset(),
		bosses.reset(),
		labResearchData.reset(),
		missionsData.reset(),
		relicData.reset()
	]);
	const upsState = getUpsState();
	await upsState.loadAll();

	send(MessageType.GET_VERSION);

	// Frames are processed FIFO: send sync first so it runs pre-reattach (no
	// save yet, settings-only emit), then reattach emits the overview once.
	send(MessageType.SYNC_APP_STATE);

	const storedSessionId = getStoredSessionId();
	if (storedSessionId) {
		markReattachPending();
		send(MessageType.REATTACH_SESSION, { session_id: storedSessionId });
	}
};
