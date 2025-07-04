import { ASSET_DATA_PATH } from '$lib/constants';
type AssetType = 'json' | 'image' | 'svg';

class AssetLoader {
	private cache: Record<string, any> = {};
	private jsonGlob = import.meta.glob('$lib/assets/data/**/*.json', { eager: true });
	private imageGlob = import.meta.glob('$lib/assets/img/**/*.png', {
		eager: true,
		query: '?url',
		import: 'default'
	});
	private svgGlob = import.meta.glob('$lib/assets/img/**/*.svg', {
		eager: true,
		query: '?raw',
		import: 'default'
	});
	private unknownIcon = this.loadImage(`${ASSET_DATA_PATH}/img/unknown.png`);

	load<T>(path: string, type: AssetType): T | undefined {
		if (this.cache[path]) {
			return this.cache[path];
		}

		let glob;
		if (type === 'json') {
			glob = this.jsonGlob;
		} else if (type === 'image') {
			glob = this.imageGlob;
		} else if (type === 'svg') {
			glob = this.svgGlob;
		} else {
			throw new Error(`Unsupported asset type: ${type}`);
		}

		if (!glob[path]) {
			console.error(`Asset not found: ${path}`);
			return undefined;
		}

		this.cache[path] = glob[path];
		return this.cache[path];
	}

	loadJson<T>(path: string): T | undefined {
		if (!path) return;
		return this.load<T>(path, 'json');
	}

	loadImage(path: string): string {
		if (!path) return this.unknownIcon;
		return this.load<any>(path.toLowerCase().replaceAll(' ', '_'), 'image');
	}

	cleanseCharacterId(character_id: string): string {
		return character_id
			.toLocaleLowerCase()
			.replace('predator_', '')
			.replace('_oilrig', '')
			.replace('raid_', '')
			.replace('summon_', '')
			.replace('_max', '')
			.replace(/_\d+$/, '')
			.replace('boss_', '')
			.replace('quest_farmer03_', '')
			.replace('_otomo', '');
	}

	loadPalImage(character_id: string, is_pal: boolean = true): string {
		if (!character_id) return this.unknownIcon;
		character_id = is_pal ? this.cleanseCharacterId(character_id) : 'commonhuman';
		let image = this.loadImage(`${ASSET_DATA_PATH}/img/${character_id}.png`);
		if (image) {
			return image;
		} else {
			image = this.loadMenuImage(character_id, is_pal);
		}
		return image || this.unknownIcon;
	}

	loadMenuImage(character_id: string, is_pal: boolean = true): string {
		if (!character_id) return this.unknownIcon;
		character_id = is_pal ? this.cleanseCharacterId(character_id) : 'commonhuman';
		const image = this.loadImage(`${ASSET_DATA_PATH}/img/t_${character_id}_icon_normal.png`);
		if (image) {
			return image;
		} else {
			console.warn(
				`Failed to load menu image for ${`${ASSET_DATA_PATH}/img/t_${character_id}_icon_normal.png`}`
			);
		}
		return this.unknownIcon;
	}

	loadSvg(path: string): string | undefined {
		return this.load<string>(path, 'svg');
	}
}

export const assetLoader = new AssetLoader();
