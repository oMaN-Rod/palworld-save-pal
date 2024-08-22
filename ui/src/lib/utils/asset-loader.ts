// src/lib/utils/assetLoader.ts

type AssetType = 'json' | 'image' | 'svg';

interface AssetLoaderOptions {
	enhanced?: boolean;
}

class AssetLoader {
	private cache: Record<string, any> = {};
	private jsonGlob = import.meta.glob('$lib/assets/data/**/*.json', { eager: true });
	private imageGlob = import.meta.glob('$lib/assets/img/**/*.png', {
		eager: true
	});
	private svgGlob = import.meta.glob('$lib/assets/img/**/*.svg', {
		eager: true,
		as: 'raw'
	});
	private enhancedImageGlob = import.meta.glob(
		'$lib/assets/img/**/*.png',
		{
			eager: true,
			query: { enhanced: true }
		}
	);

	async load<T>(path: string, type: AssetType, options: AssetLoaderOptions = {}): Promise<T> {
		if (this.cache[path]) {
			return this.cache[path];
		}

		let glob;
		if (type === 'json') {
			glob = this.jsonGlob;
		} else if (type === 'image') {
			glob = options.enhanced ? this.enhancedImageGlob : this.imageGlob;
		} else if (type === 'svg') {
			glob = this.svgGlob;
		} else {
			throw new Error(`Unsupported asset type: ${type}`);
		}

		const module = glob[path];
		if (!module) {
			throw new Error(`Asset not found: ${path}`);
		}

		this.cache[path] = module.default || module;
		return this.cache[path];
	}

	async loadJson<T>(path: string): Promise<T> {
		return this.load<T>(path, 'json');
	}

	async loadImage(path: string, enhanced: boolean = true): Promise<string> {
		const image = await this.load<any>(path, 'image', { enhanced });
		return enhanced && image && image.src ? image.src : image;
	}

	async loadSvg(path: string): Promise<string> {
		return this.load<string>(path, 'svg');
	}

	async loadAllImages(
		directory: string,
		enhanced: boolean = true
	): Promise<Record<string, string>> {
		const glob = enhanced ? this.enhancedImageGlob : this.imageGlob;

		const images: Record<string, string> = {};
		for (const [path, module] of Object.entries(glob)) {
			if (path.startsWith(directory)) {
				images[path] = (module as any).default || module;
			}
		}
		return images;
	}
}

export const assetLoader = new AssetLoader();
