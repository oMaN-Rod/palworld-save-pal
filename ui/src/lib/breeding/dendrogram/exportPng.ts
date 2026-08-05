/**
 * exportPng — rasterize the breeding dendrogram SVG to a PNG blob, for
 * download or clipboard.
 *
 * Strategy: measure the tree's NATURAL layout bounds (ignores current
 * zoom/pan → the exported image always shows the full tree, fitted), deep
 * clone the SVG, inline the bundled pal-icon assets as data URLs (so the
 * rasterization can't hit a tainted/cross-origin canvas), drop the d3-zoom
 * transform, wrap the content in a translate that fits the bounds + margin
 * into the output, then draw the serialized SVG onto a 2× canvas.
 *
 * The SVG never leaves the document and no new deps are added: canvas
 * toBlob + ClipboardItem are both browser built-ins. Only works on the SVG
 * rendered by `DendrogramEngine` (its `g.dendro-zoom-layer` holds the tree).
 */
import { DENDRO_COLORS } from './constants';

export interface ExportBounds {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface ExportFit {
	dx: number;
	dy: number;
	width: number;
	height: number;
}

/** Pure layout math (unit-testable): map a tree bbox into a `margin`-padded canvas. */
export function fitBounds(bbox: ExportBounds, margin: number): ExportFit {
	const width = Math.max(bbox.width + margin * 2, 1);
	const height = Math.max(bbox.height + margin * 2, 1);
	return { dx: margin - bbox.x, dy: margin - bbox.y, width, height };
}

/** Pure name helper (unit-testable): "LazyDragon_Electric" → "lazydragon-electric". */
export function slugify(s: string): string {
	const slug = s
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
	return slug || 'dendrogram';
}

function toDataUrl(url: string): Promise<string> {
	return fetch(url)
		.then((res) => res.blob())
		.then(
			(blob) =>
				new Promise<string>((resolve, reject) => {
					const reader = new FileReader();
					reader.onload = () => resolve(reader.result as string);
					reader.onerror = () => reject(reader.error);
					reader.readAsDataURL(blob);
				})
		);
}

function loadImage(src: string): Promise<HTMLImageElement> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve(img);
		img.onerror = () => reject(new Error('Failed to decode exported SVG'));
		img.src = src;
	});
}

export interface ExportPngOptions {
	/** Raster resolution multiplier (2 = crisp on a 2× display / sharable PNG). */
	scale?: number;
	/** Padding around the tree in output pixels (before scaling). */
	margin?: number;
}

/**
 * Rasterize the displayed dendrogram to a PNG blob. The whole tree is always
 * included and auto-fitted — the current zoom/pan state is discarded.
 */
export async function exportTreeToPng(
	svgEl: SVGSVGElement,
	options: ExportPngOptions = {}
): Promise<Blob> {
	const { scale = 2, margin = 32 } = options;

	const zoomLayer = svgEl.querySelector<SVGGElement>('.dendro-zoom-layer');
	if (!zoomLayer || zoomLayer.childElementCount === 0) {
		throw new Error('Dendrogram is not rendered');
	}
	// getBBox on the layer returns its children's bounds in the layer's LOCAL
	// coordinates — i.e. the natural tree layout, unaffected by the d3-zoom
	// transform on the same element.
	const bbox = zoomLayer.getBBox();
	if (bbox.width <= 0 || bbox.height <= 0) {
		throw new Error('Dendrogram has no visible tree');
	}
	const fit = fitBounds({ x: bbox.x, y: bbox.y, width: bbox.width, height: bbox.height }, margin);

	const clone = svgEl.cloneNode(true) as SVGSVGElement;
	const cloneLayer = clone.querySelector<SVGGElement>('.dendro-zoom-layer');
	if (!cloneLayer) throw new Error('Failed to clone dendrogram');

	// Pin the output document size, drop the zoom transform, fit the tree.
	cloneLayer.removeAttribute('transform');
	cloneLayer.setAttribute('transform', `translate(${fit.dx},${fit.dy})`);
	clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
	clone.setAttribute('width', String(fit.width));
	clone.setAttribute('height', String(fit.height));
	clone.setAttribute('viewBox', `0 0 ${fit.width} ${fit.height}`);

	// Opaque background so the PNG is readable on any viewer.
	const background = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
	background.setAttribute('x', '0');
	background.setAttribute('y', '0');
	background.setAttribute('width', '100%');
	background.setAttribute('height', '100%');
	background.setAttribute('fill', DENDRO_COLORS.bgCard);
	clone.insertBefore(background, clone.firstChild);

	// Inline the bundled pal-icon assets so the SVG document is self-contained
	// and the canvas can't be tainted by a (same-origin but async) fetch race.
	const icons = Array.from(clone.querySelectorAll<SVGImageElement>('image[href]'));
	await Promise.all(
		icons.map(async (img) => {
			const href = img.getAttribute('href');
			if (!href || href.startsWith('data:')) return;
			try {
				img.setAttribute('href', await toDataUrl(href));
			} catch {
				// Keep the original URL — worst case the icon is blank in the PNG.
			}
		})
	);

	const svgString = new XMLSerializer().serializeToString(clone);
	const svgUrl = URL.createObjectURL(new Blob([svgString], { type: 'image/svg+xml;charset=utf-8' }));
	try {
		const image = await loadImage(svgUrl);
		const canvas = document.createElement('canvas');
		canvas.width = Math.round(fit.width * scale);
		canvas.height = Math.round(fit.height * scale);
		const ctx = canvas.getContext('2d');
		if (!ctx) throw new Error('Canvas 2D is not supported');
		ctx.scale(scale, scale);
		ctx.drawImage(image, 0, 0);
		const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
		if (!blob) throw new Error('PNG encoding failed');
		return blob;
	} finally {
		URL.revokeObjectURL(svgUrl);
	}
}

/** Trigger a browser download of the blob (same a[download] pattern as blueprintHandler). */
export function downloadPng(blob: Blob, filename: string): void {
	const url = URL.createObjectURL(blob);
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = filename;
	anchor.click();
	URL.revokeObjectURL(url);
}

/**
 * Put the PNG on the system clipboard. Returns false when the browser can't
 * write images (insecure context or missing ClipboardItem support).
 */
export async function copyPngToClipboard(blob: Blob): Promise<boolean> {
	if (typeof ClipboardItem === 'undefined' || !navigator.clipboard?.write) return false;
	try {
		await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
		return true;
	} catch {
		return false;
	}
}