import { getSocketState } from '$states';
import { MessageType } from '$types';
import {
	activeSkillsData,
	buildingsData,
	elementsData,
	expData,
	itemsData,
	mapObjects,
	palsData,
	passiveSkillsData,
	presetsData,
	technologiesData,
	workSuitabilityData
} from '.';

const ws = getSocketState();

export const bootstrap = async () => {
	while (!ws.connected) {
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
	await presetsData.reset();
	await palsData.reset();
	await activeSkillsData.reset();
	await passiveSkillsData.reset();
	await technologiesData.reset();
	await elementsData.reset();
	await expData.reset();
	await itemsData.reset();
	await workSuitabilityData.reset();
	await buildingsData.reset();
	await mapObjects.reset();
	ws.send(JSON.stringify({ type: MessageType.GET_VERSION }));
	ws.send(JSON.stringify({ type: MessageType.SYNC_APP_STATE }));
};
