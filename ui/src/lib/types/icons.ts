import { assetLoader } from '$utils';
export const ASSET_DATA_PATH = '/src/lib/assets';

export const staticIcons = {
	foodIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Food.webp`),
	hpIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Heart.webp`),
	sadIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Cattiva_Pleading.webp`),
	alphaIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Alpha.webp`),
	leftClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/MouseButtonLeft.webp`),
	rightClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/MouseButtonRight.webp`),
	middleClickIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/MouseWheelButton.webp`),
	ctrlIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Ctrl.webp`),
	rIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_R.webp`),
	qIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Q.webp`),
	eIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_E.webp`),
	plusIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Num_Plus.webp`),
	minusIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_Num_Minus.webp`),
	f5Icon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/Keyboard_F5.webp`),
	weightIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/weight.webp`),
	attackIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/attack.webp`),
	defenseIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/defense.webp`),
	workSpeedIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/work_speed.webp`),
	staminaIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/stamina.webp`),
	predatorIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/predator.webp`),
	oilrigIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/oilrig.webp`),
	unknownIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/unknown.webp`),
	unknownEggIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/unknown_egg.webp`),
	altarIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/altar.webp`),
	luckyIcon: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/lucky.webp`),
	pspWhite: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/app/psp_white.webp`),
	lamball: assetLoader.loadImage(`${ASSET_DATA_PATH}/img/sheepball.webp`)
};
