import {
	activeSkillsData,
	elementsData,
	expData,
	itemsData,
	palsData,
	passiveSkillsData,
	presetsData
} from '.';

export const bootstrap = async () => {
	await presetsData.reset();
	await palsData.reset();
	await activeSkillsData.reset();
	await passiveSkillsData.reset();
	await elementsData.reset();
	await expData.reset();
	await itemsData.reset();
	// await uiCommonData.reset();
};
