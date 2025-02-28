import { assetLoader } from '$utils';

export const ASSET_DATA_PATH = '/src/lib/assets';

export const MAX_LEVEL = 60;

export const staticIcons = {
	foodIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Food.png`),
	hpIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Heart.png`),
	sadIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Cattiva_Pleading.png`),
	alphaIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Alpha.png`),
	leftClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/MouseButtonLeft.png`),
	rightClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/MouseButtonRight.png`),
	middleClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/MouseWheelButton.png`),
	ctrlIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Ctrl.png`),
	rIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_R.png`),
	qIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Q.png`),
	eIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_E.png`),
	plusIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Num_Plus.png`),
	minusIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Num_Minus.png`),
	f5Icon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_F5.png`),
	weightIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/weight.png`),
	attackIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/attack.png`),
	defenseIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/defense.png`),
	workSpeedIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/work_speed.png`),
	staminaIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/stamina.png`),
	predatorIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/predator.png`),
	oilrigIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/oilrig.png`),
	unknownIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/unknown.png`),
	altarIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/altar.png`),
	luckyIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/lucky.png`),
	pspWhite: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/app/psp_white.png`),
	lamball: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/sheepball.png`)
};
