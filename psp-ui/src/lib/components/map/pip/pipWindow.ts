
export type DocumentPictureInPictureWindow = Window;

export interface DocumentPictureInPictureRequestOptions {
	width?: number;
	height?: number;
	disallowReturnToOpener?: boolean;
}

interface DocumentPictureInPictureApi extends EventTarget {
	requestWindow(
		options?: DocumentPictureInPictureRequestOptions
	): Promise<DocumentPictureInPictureWindow>;
	readonly window: DocumentPictureInPictureWindow | null;
}

declare global {
	interface Window {
		documentPictureInPicture?: DocumentPictureInPictureApi;
	}
}

export function isDocumentPipSupported(): boolean {
	return typeof window !== 'undefined' && 'documentPictureInPicture' in window;
}

function copyStylesheetsInto(targetDocument: Document): void {
	for (const styleSheet of Array.from(document.styleSheets)) {
		try {
			const cssRules = Array.from(styleSheet.cssRules)
				.map((rule) => rule.cssText)
				.join('');
			const style = targetDocument.createElement('style');
			style.textContent = cssRules;
			targetDocument.head.appendChild(style);
		} catch {
			if (!styleSheet.href) continue;
			const link = targetDocument.createElement('link');
			link.rel = 'stylesheet';
			link.type = styleSheet.type;
			if (styleSheet.media) link.media = styleSheet.media.toString();
			link.href = styleSheet.href;
			targetDocument.head.appendChild(link);
		}
	}
}

// Svelte compiles most `on*` attributes into a single listener on the main
// document that walks the propagation path invoking each node's `__<type>`
// handler property. A subtree moved into the pip document never reaches that
// listener, so its buttons go dead — this repeats the walk from the pip
// document, over the same handler properties, bounded by the moved subtree.
// Kept in step with svelte's DELEGATED_EVENTS (see svelte/src/utils.js).
const DELEGATED_EVENTS = [
	'beforeinput',
	'click',
	'change',
	'dblclick',
	'contextmenu',
	'focusin',
	'focusout',
	'input',
	'keydown',
	'keyup',
	'mousedown',
	'mousemove',
	'mouseout',
	'mouseover',
	'mouseup',
	'pointerdown',
	'pointermove',
	'pointerout',
	'pointerover',
	'pointerup',
	'touchend',
	'touchmove',
	'touchstart'
] as const;

type DelegatedNode = Node & {
	disabled?: boolean;
	[key: string]: unknown;
};

function invokeDelegated(type: string, root: HTMLElement, event: Event): void {
	const target = event.target;
	if (!(target instanceof Node) || !root.contains(target)) return;

	let node: Node | null = target;
	let currentTarget: Node = target;
	Object.defineProperty(event, 'currentTarget', {
		configurable: true,
		get: () => currentTarget
	});

	try {
		while (node) {
			currentTarget = node;
			const handler = (node as DelegatedNode)['__' + type];
			const enabled = !(node as DelegatedNode).disabled || event.target === node;
			if (typeof handler === 'function' && enabled) {
				(handler as (this: Node, event: Event) => void).call(node, event);
			}
			if (event.cancelBubble || node === root) break;
			node = node.parentNode ?? (node as Partial<ShadowRoot>).host ?? null;
		}
	} finally {
		Reflect.deleteProperty(event, 'currentTarget');
	}
}

const PASSIVE_EVENTS: readonly string[] = ['touchstart', 'touchmove'];

function bridgeDelegatedEvents(pipDocument: Document, root: HTMLElement): () => void {
	const listeners = DELEGATED_EVENTS.map((type) => {
		const options = { passive: PASSIVE_EVENTS.includes(type) };
		const listener = (event: Event) => invokeDelegated(type, root, event);
		pipDocument.addEventListener(type, listener, options);
		return () => pipDocument.removeEventListener(type, listener);
	});
	return () => listeners.forEach((remove) => remove());
}

export interface OpenDocPipOptions {
	width: number;
	height: number;
	onDismiss: () => void;
	onResize?: (width: number, height: number) => void;
}

export async function openDocPipWindow(
	contentEl: HTMLElement,
	options: OpenDocPipOptions
): Promise<DocumentPictureInPictureWindow> {
	const api = window.documentPictureInPicture;
	if (!api) throw new Error('Document Picture-in-Picture is not supported');

	const pipWindow = await api.requestWindow({
		width: options.width,
		height: options.height,
		disallowReturnToOpener: true
	});
	copyStylesheetsInto(pipWindow.document);
	const hostStyle = pipWindow.document.createElement('style');
	hostStyle.textContent = 'html, body { height: 100%; margin: 0; overflow: hidden; }';
	pipWindow.document.head.appendChild(hostStyle);
	pipWindow.document.body.appendChild(contentEl);
	const unbridge = bridgeDelegatedEvents(pipWindow.document, contentEl);

	pipWindow.addEventListener(
		'pagehide',
		() => {
			unbridge();
			options.onDismiss();
		},
		{ once: true }
	);
	if (options.onResize) {
		pipWindow.addEventListener('resize', () =>
			options.onResize?.(pipWindow.innerWidth, pipWindow.innerHeight)
		);
	}

	return pipWindow;
}

export function closeDocPipWindow(pipWindow: Window): void {
	pipWindow.close();
}
