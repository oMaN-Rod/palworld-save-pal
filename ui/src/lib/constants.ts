import { assetLoader } from '$utils';

export const ASSET_DATA_PATH = '/src/lib/assets';

export const MAX_LEVEL = 60;

export const staticIcons = {
	foodIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/Food.png`),
	hpIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/Heart.png`),
	sadIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/Cattiva_Pleading.png`),
	alphaIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/Alpha.png`),
	rightClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/actions/MouseButtonRight.png`),
	middleClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/actions/MouseWheelButton.png`),
	ctrlIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/actions/Keyboard_Ctrl.png`),
	weightIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/weight.png`),
	attackIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/stats/attack.png`),
	defenseIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/stats/defense.png`),
	workSpeedIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/stats/work_speed.png`),
	staminaIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/stats/stamina.png`),
	predatorIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/predator.png`),
	oilrigIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/oilrig.png`),
	unknownIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/unknown.png`),
	altarIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/altar.png`),
	luckyIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/icons/lucky.png`)
};
