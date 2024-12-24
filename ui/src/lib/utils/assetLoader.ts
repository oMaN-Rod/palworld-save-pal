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
		return this.load<T>(path, 'json');
	}

	loadImage(path: string): string | undefined {
		return this.load<any>(path.toLowerCase(), 'image');
	}

	loadSvg(path: string): string | undefined {
		return this.load<string>(path, 'svg');
	}
}

export const assetLoader = new AssetLoader();
