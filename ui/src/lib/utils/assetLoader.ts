import { ASSET_DATA_PATH } from '$lib/constants';
type AssetType = 'json' | 'image' | 'svg' | 'webp';

class AssetLoader {
	private cache: Record<string, any> = {};
	private jsonGlob = import.meta.glob('$lib/assets/data/**/*.json', { eager: true });
	private imageGlob = import.meta.glob('$lib/assets/img/**/*.webp', {
		eager: true,
		query: '?url',
		import: 'default'
	});
	private svgGlob = import.meta.glob('$lib/assets/img/**/*.svg', {
		eager: true,
		query: '?raw',
		import: 'default'
	});
	private webpGlob = import.meta.glob('$lib/assets/img/**/*.webp', {
		eager: true,
		query: '?url',
		import: 'default'
	});
	private unknownIcon = this.loadImage(`${ASSET_DATA_PATH}/img/unknown.webp`);

	load<T>(path: string, type: AssetType): T | undefined {
		if (this.cache[path]) {
			return this.cache[path];
		}

		let glob;
		switch (type) {
			case 'json':
				glob = this.jsonGlob;
				break;
			case 'image':
				glob = this.imageGlob;
				break;
			case 'svg':
				glob = this.svgGlob;
				break;
			case 'webp':
				glob = this.webpGlob;
				break;
			default:
				throw new Error(`Unsupported asset type: ${type}`);
		}

		if (!glob[path]) {
			console.error(`Asset not found: ${type} ${path}`);
			return undefined;
		}

		this.cache[path] = glob[path];
		return this.cache[path];
	}

	loadJson<T>(path: string): T | undefined {
		if (!path) return;
		return this.load<T>(path, 'json');
	}

	loadImage(path: string, type: AssetType = 'webp'): string {
		return this.tryImage(path, type) ?? this.unknownIcon;
	}

	// like loadImage, but reports a miss as undefined so callers can try
	// another candidate path before settling on the unknown icon
	private tryImage(path: string, type: AssetType = 'webp'): string | undefined {
		if (!path) return undefined;
		return this.load<any>(path.toLowerCase().replaceAll(' ', '_'), type);
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
		if (!is_pal) return this.loadMenuImage(character_id, false);
		const id = this.cleanseCharacterId(character_id);
		return (
			this.tryImage(`${ASSET_DATA_PATH}/img/${id}.webp`) ?? this.loadMenuImage(character_id, is_pal)
		);
	}

	loadMenuImage(character_id: string, is_pal: boolean = true): string {
		if (!character_id) return this.unknownIcon;
		if (is_pal) {
			const id = this.cleanseCharacterId(character_id);
			return this.tryImage(`${ASSET_DATA_PATH}/img/t_${id}_icon_normal.webp`) ?? this.unknownIcon;
		}
		// humans: prefer the NPC's own portrait, fall back to the generic human icon
		const id = character_id.toLowerCase();
		return (
			this.tryImage(`${ASSET_DATA_PATH}/img/t_${id}_icon_normal.webp`) ??
			this.tryImage(`${ASSET_DATA_PATH}/img/t_commonhuman_icon_normal.webp`) ??
			this.unknownIcon
		);
	}

	loadSvg(path: string): string | undefined {
		return this.load<string>(path, 'svg');
	}
}

export const assetLoader = new AssetLoader();
