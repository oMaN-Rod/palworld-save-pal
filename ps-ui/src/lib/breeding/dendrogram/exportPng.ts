import { resolveDendroColors } from './constants';

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

export function fitBounds(bbox: ExportBounds, margin: number): ExportFit {
	const width = Math.max(bbox.width + margin * 2, 1);
	const height = Math.max(bbox.height + margin * 2, 1);
	return { dx: margin - bbox.x, dy: margin - bbox.y, width, height };
}

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
	scale?: number;
	margin?: number;
}

export async function exportTreeToPng(
	svgEl: SVGSVGElement,
	options: ExportPngOptions = {}
): Promise<Blob> {
	const { scale = 2, margin = 32 } = options;

	const zoomLayer = svgEl.querySelector<SVGGElement>('.dendro-zoom-layer');
	if (!zoomLayer || zoomLayer.childElementCount === 0) {
		throw new Error('Dendrogram is not rendered');
	}
	const bbox = zoomLayer.getBBox();
	if (bbox.width <= 0 || bbox.height <= 0) {
		throw new Error('Dendrogram has no visible tree');
	}
	const fit = fitBounds({ x: bbox.x, y: bbox.y, width: bbox.width, height: bbox.height }, margin);

	const clone = svgEl.cloneNode(true) as SVGSVGElement;
	const cloneLayer = clone.querySelector<SVGGElement>('.dendro-zoom-layer');
	if (!cloneLayer) throw new Error('Failed to clone dendrogram');

	cloneLayer.removeAttribute('transform');
	cloneLayer.setAttribute('transform', `translate(${fit.dx},${fit.dy})`);
	clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
	clone.setAttribute('width', String(fit.width));
	clone.setAttribute('height', String(fit.height));
	clone.setAttribute('viewBox', `0 0 ${fit.width} ${fit.height}`);

	const colors = resolveDendroColors();
	const background = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
	background.setAttribute('x', '0');
	background.setAttribute('y', '0');
	background.setAttribute('width', '100%');
	background.setAttribute('height', '100%');
	background.setAttribute('fill', colors.bgCard);
	clone.insertBefore(background, clone.firstChild);

	const icons = Array.from(clone.querySelectorAll<SVGImageElement>('image[href]'));
	await Promise.all(
		icons.map(async (img) => {
			const href = img.getAttribute('href');
			if (!href || href.startsWith('data:')) return;
			try {
				img.setAttribute('href', await toDataUrl(href));
			} catch {
			}
		})
	);

	const svgString = new XMLSerializer().serializeToString(clone);
	const svgUrl = URL.createObjectURL(
		new Blob([svgString], { type: 'image/svg+xml;charset=utf-8' })
	);
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

export function downloadPng(blob: Blob, filename: string): void {
	const url = URL.createObjectURL(blob);
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = filename;
	anchor.click();
	URL.revokeObjectURL(url);
}

export async function copyPngToClipboard(blob: Blob): Promise<boolean> {
	if (typeof ClipboardItem === 'undefined' || !navigator.clipboard?.write) return false;
	try {
		await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
		return true;
	} catch {
		return false;
	}
}
