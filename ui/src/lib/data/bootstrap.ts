import { getSocketState } from '$states';
import { MessageType } from '$types';
import {
	activeSkillsData,
	buildingsData,
	elementsData,
	expData,
	itemsData,
	palsData,
	passiveSkillsData,
	presetsData,
	technologiesData,
	workSuitabilityData
} from '.';

const ws = getSocketState();

export const bootstrap = async () => {
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
	ws.send(JSON.stringify({ type: MessageType.GET_VERSION }));
	ws.send(JSON.stringify({ type: MessageType.SYNC_APP_STATE }));
};
