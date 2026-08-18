/**
 * DendrogramEngine — framework-agnostic D3 renderer for ONE breeding chain.
 *
 * Ported from PalSavTools. Integration changes: `assetUrl(icon)` →
 * `assetLoader.loadMenuImage(character_id)` for PSP's bundled-image model, and
 * layout is delegated to `./layouts` so the same renderer serves the
 * dendrogram, smooth, bracket and radial views.
 *
 * The tree reads target-first: the target pal is the root and source pals are
 * the leaves.
 */
import { select, type Selection } from 'd3-selection';
import { transition } from 'd3-transition';
import { zoom, zoomIdentity, zoomTransform, type D3ZoomEvent, type ZoomBehavior } from 'd3-zoom';

import { assetLoader } from '$lib/utils/assetLoader';
import { DENDRO_COLORS, DENDRO_CONFIG, resolveDendroColors, type DendroColors } from './constants';
import {
	computeLayout,
	type LayoutMode,
	type PositionedLink,
	type PositionedNode
} from './layouts';
import type { NodeHoverCallback, NodeSelectCallback, TreeNode } from './types';

export class DendrogramEngine {
	private svg: Selection<SVGSVGElement, unknown, null, undefined>;
	private zoomLayer: Selection<SVGGElement, unknown, null, undefined>;
	private linksLayer: Selection<SVGGElement, unknown, null, undefined>;
	private nodesLayer: Selection<SVGGElement, unknown, null, undefined>;
	private zoomBehavior: ZoomBehavior<SVGSVGElement, unknown>;

	private layoutNodes: PositionedNode[] = [];
	private layoutLinks: PositionedLink[] = [];
	private layoutIndex = new Map<string, PositionedNode>();
	private currentTransform = zoomIdentity;
	private colors: DendroColors = DENDRO_COLORS;

	selectedId: string | null = null;
	hoveredId: string | null = null;

	matchedPassives: Set<string> = new Set();
	passiveName: (asset: string) => string = (s) => s;
	layoutMode: LayoutMode = 'dendrogram';
	/** Last tree handed to `render`, so a layout switch can re-lay it out. */
	private currentTree: TreeNode | null = null;

	callbacks: {
		onSelect?: NodeSelectCallback;
		onHover?: NodeHoverCallback;
	} = {};

	constructor(svgEl: SVGSVGElement) {
		this.svg = select(svgEl);
		this.defineFilters();
		this.zoomLayer = this.svg.append('g').attr('class', 'dendro-zoom-layer');
		this.linksLayer = this.zoomLayer.append('g').attr('class', 'dendro-links');
		this.nodesLayer = this.zoomLayer.append('g').attr('class', 'dendro-nodes');

		this.zoomBehavior = zoom<SVGSVGElement, unknown>()
			.scaleExtent([DENDRO_CONFIG.zoom.min, DENDRO_CONFIG.zoom.max])
			.on('zoom', (event: D3ZoomEvent<SVGSVGElement, unknown>) => {
				this.currentTransform = event.transform;
				this.zoomLayer.attr('transform', event.transform.toString());
			});
		this.svg.call(this.zoomBehavior);
		this.svg.on('dblclick.zoom', null);
	}

	/**
	 * Lay out and draw `treeRoot`. Selection and hover survive a re-render when
	 * the node still exists, so re-rendering (a theme change, a layout switch)
	 * does not silently clear what the user picked.
	 */
	render(treeRoot: TreeNode): void {
		this.colors = resolveDendroColors();
		this.currentTree = treeRoot;

		const { nodes, links, index } = computeLayout(treeRoot, this.layoutMode);
		this.layoutNodes = nodes;
		this.layoutLinks = links;
		this.layoutIndex = index;

		if (this.selectedId && !index.has(this.selectedId)) this.selectedId = null;
		if (this.hoveredId && !index.has(this.hoveredId)) this.hoveredId = null;

		this.drawLinks();
		this.drawNodes();
	}

	/** Switch view without rebuilding the tree. No-op if the mode is unchanged. */
	setLayout(mode: LayoutMode): void {
		if (mode === this.layoutMode) return;
		this.layoutMode = mode;
		if (this.currentTree) this.render(this.currentTree);
	}

	private drawLinks(): void {
		const sel = this.linksLayer
			.selectAll<SVGPathElement, PositionedLink>('path.dendro-link')
			.data(this.layoutLinks, (d) => `${d.source.node.id}->${d.target.node.id}`);

		sel.exit().remove();
		const enter = sel
			.enter()
			.append('path')
			.attr('class', 'dendro-link')
			.attr('fill', 'none')
			.attr('stroke', this.colors.link)
			.attr('stroke-width', 1.5)
			.attr('stroke-linecap', 'round')
			.attr('stroke-linejoin', 'round');

		enter
			.merge(sel)
			.attr('d', (d) => d.path)
			.attr('stroke', (d) => {
				if (d.source.node.id === this.selectedId || d.target.node.id === this.selectedId) {
					return this.colors.linkHighlight;
				}
				return this.colors.link;
			})
			.attr('stroke-width', (d) => {
				if (d.source.node.id === this.selectedId || d.target.node.id === this.selectedId) {
					return 2.5;
				}
				return 1.5;
			})
			.attr('opacity', (d) => {
				if (!this.selectedId && !this.hoveredId) return 0.7;
				if (
					d.source.node.id === this.selectedId ||
					d.target.node.id === this.selectedId ||
					d.source.node.id === this.hoveredId ||
					d.target.node.id === this.hoveredId
				) {
					return 1;
				}
				return 0.2;
			});
	}

	private drawNodes(): void {
		const sel = this.nodesLayer
			.selectAll<SVGGElement, PositionedNode>('g.dendro-node')
			.data(this.layoutNodes, (d) => d.node.id);

		sel.exit().remove();

		const enter = sel.enter().append('g').attr('class', 'dendro-node').style('cursor', 'pointer');

		enter
			.append('rect')
			.attr('class', 'dendro-card')
			.attr('width', (d) => d.w)
			.attr('height', DENDRO_CONFIG.nodeHeight)
			.attr('rx', 10)
			.attr('ry', 10)
			.attr('stroke-width', 2)
			.attr('filter', 'url(#dendro-card-shadow)');

		enter
			.append('clipPath')
			.attr('class', 'dendro-icon-clip')
			.attr('id', (d) => `clip-${cssEscape(d.node.id)}`)
			.append('rect')
			.attr('x', DENDRO_CONFIG.iconPadding)
			.attr('y', DENDRO_CONFIG.iconPadding)
			.attr('width', DENDRO_CONFIG.iconSize)
			.attr('height', DENDRO_CONFIG.iconSize)
			.attr('rx', 6);

		enter
			.append('image')
			.attr('class', 'dendro-icon')
			.attr('x', DENDRO_CONFIG.iconPadding)
			.attr('y', DENDRO_CONFIG.iconPadding)
			.attr('width', DENDRO_CONFIG.iconSize)
			.attr('height', DENDRO_CONFIG.iconSize)
			.attr('preserveAspectRatio', 'xMidYMid slice')
			.attr('clip-path', (d) => `url(#clip-${cssEscape(d.node.id)})`)
			.attr('href', (d) => assetLoader.loadMenuImage(d.node.character_id));

		enter
			.append('text')
			.attr('class', 'dendro-name')
			.attr('x', DENDRO_CONFIG.iconPadding * 2 + DENDRO_CONFIG.iconSize + 5)
			.attr('y', 20)
			.attr('fill', this.colors.inkPrimary)
			.attr('font-size', 11)
			.attr('font-weight', 600)
			.attr('dominant-baseline', 'middle');

		enter
			.append('text')
			.attr('class', 'dendro-gender')
			.attr('y', 20)
			.attr('font-size', 11)
			.attr('dominant-baseline', 'middle');

		enter.append('g').attr('class', 'dendro-passives');

		enter
			.append('circle')
			.attr('class', 'dendro-source-dot')
			.attr('cx', (d) => d.w - 8)
			.attr('cy', DENDRO_CONFIG.nodeHeight - 8)
			.attr('r', 4);

		enter
			.append('text')
			.attr('class', 'dendro-step')
			.attr('x', (d) => d.w - 8)
			.attr('y', 13)
			.attr('text-anchor', 'end')
			.attr('font-size', 8)
			.attr('fill', this.colors.inkDim)
			.attr('dominant-baseline', 'middle');

		enter
			.append('text')
			.attr('class', 'dendro-target-badge')
			.attr('x', (d) => d.w - 8)
			.attr('y', DENDRO_CONFIG.nodeHeight - 8)
			.attr('text-anchor', 'end')
			.attr('font-size', 8)
			.attr('font-weight', 700)
			.attr('fill', this.colors.accentTarget)
			.attr('dominant-baseline', 'middle')
			.attr('opacity', 0);

		const merged = enter.merge(sel);

		merged
			.attr('transform', (d) => `translate(${d.x},${d.y - DENDRO_CONFIG.nodeHeight / 2})`)
			.attr('opacity', (d) => {
				// Dim everything not on the highlighted node's own edges, matching
				// how the links fade, so focus reads across marks and connectors.
				if (!this.selectedId && !this.hoveredId) return 1;
				return this.isAdjacentToFocus(d.node.id) ? 1 : 0.35;
			});

		merged
			.select<SVGRectElement>('.dendro-card')
			.attr('fill', (d) => {
				if (d.node.id === this.selectedId) return this.colors.bgCardSelected;
				if (d.node.id === this.hoveredId) return this.colors.bgCardHover;
				if (d.node.isBred && !d.node.isTarget) return this.colors.bgCardBred;
				return this.colors.bgCard;
			})
			.attr('stroke', (d) => {
				if (d.node.isTarget) return this.colors.accentTarget;
				if (d.node.id === this.selectedId) return this.colors.accent;
				if (d.node.id === this.hoveredId) return this.colors.accentLight;
				return this.colors.line;
			})
			.attr('stroke-width', (d) => (d.node.isTarget ? 2.5 : 2));

		merged.select<SVGTextElement>('.dendro-name').text((d) => d.node.display);

		merged
			.select<SVGTextElement>('.dendro-target-badge')
			.text((d) => (d.node.isTarget ? 'TARGET' : ''))
			.attr('opacity', (d) => (d.node.isTarget ? 1 : 0));

		const passiveName = this.passiveName;
		const matchedPassives = this.matchedPassives;
		const colors = this.colors;
		const glyphX = this.genderGlyphX.bind(this);
		const iconTextX = DENDRO_CONFIG.iconPadding * 2 + DENDRO_CONFIG.iconSize + 5;
		merged.each(function (this: SVGGElement, d: PositionedNode) {
			// --- Name + gender glyph ---------------------------------------
			// Fit the display name to the card's usable width (measured, not
			// a char-count guess — CJK/wide glyphs are wider than an ASCII
			// estimate), reserving a slot for the gender glyph and the
			// right-edge step/target badges. Long names shrink instead of
			// overlapping the glyph or spilling past the card.
			const cardW = d.w;
			const rightReserve = 30;
			const genderSlot = genderGlyph(d.node.gender) ? 16 : 0;
			const nameMaxW = Math.max(cardW - iconTextX - rightReserve - genderSlot, 20);

			const nameEl = select(this).select<SVGTextElement>('.dendro-name');
			const nameNode = nameEl.node();
			if (nameNode) {
				try {
					fitWidth(nameNode, nameMaxW);
				} catch {
					// not laid out yet — keep the unmeasured text
				}
			}
			const nameW = nameNode ? nameNode.getComputedTextLength() : 0;
			const genderEl = select(this).select<SVGTextElement>('.dendro-gender');
			const genderX = iconTextX + (nameW > 0 ? nameW + 5 : glyphX(d.node.display) - iconTextX);
			genderEl
				.attr('x', genderX)
				.text(genderGlyph(d.node.gender))
				.attr('fill', genderColor(colors, d.node.gender));

			// --- Passive chips line -----------------------------------------
			const g = select(this).select<SVGGElement>('g.dendro-passives');
			const chipX0 = iconTextX;
			const chipY = 38;
			const passiveMaxW = Math.max(cardW - chipX0 - rightReserve, 30);
			const visible = d.node.passives.slice(0, 3);
			const overflow = d.node.passives.length - visible.length;

			g.selectAll<SVGTextElement, string>('text').remove();
			let label = '';
			let allMatched = false;
			if (overflow > 0) {
				const first = visible[0];
				const name = first ? passiveName(first) : '';
				const short = name && name.length > 10 ? name.slice(0, 9) + '…' : name;
				label = short ? `${short} +${overflow}` : '';
				allMatched = visible.every((p) => matchedPassives.has(p));
			} else if (visible.length) {
				const parts = visible.map((p) => {
					const name = passiveName(p);
					return name.length > 10 ? name.slice(0, 9) + '…' : name;
				});
				label = parts.join(', ');
				allMatched = visible.every((p) => matchedPassives.has(p));
			}
			if (label) {
				const t = g
					.append('text')
					.attr('x', chipX0)
					.attr('y', chipY)
					.attr('font-size', 8)
					.attr('font-weight', allMatched ? 600 : 400)
					.attr('fill', allMatched ? colors.passiveMatched : colors.inkSecondary)
					.attr('dominant-baseline', 'middle')
					.text(label)
					.node();
				if (t) {
					try {
						fitWidth(t, passiveMaxW);
					} catch {
						// not laid out yet
					}
				}
			}
		});

		merged
			.select<SVGCircleElement>('.dendro-source-dot')
			.attr('fill', (d) =>
				d.node.sourceType ? this.colors[d.node.sourceType] : this.colors.bgDeep
			)
			.attr('opacity', (d) => (d.node.sourceType ? 1 : 0));

		merged
			.select<SVGTextElement>('.dendro-step')
			.text((d) =>
				d.node.isBred && d.node.stepIndex !== undefined ? `#${d.node.stepIndex + 1}` : ''
			)
			.attr('opacity', (d) => (d.node.isBred ? 1 : 0));
	}

	/**
	 * Soft drop shadow, declared once. Kept as an SVG filter rather than a CSS
	 * one so the PNG export — which serializes the SVG with no stylesheet —
	 * rasterizes with the same depth the on-screen tree has.
	 */
	private defineFilters(): void {
		const defs = this.svg.append('defs');
		const shadow = defs
			.append('filter')
			.attr('id', 'dendro-card-shadow')
			.attr('x', '-20%')
			.attr('y', '-20%')
			.attr('width', '140%')
			.attr('height', '140%');
		shadow
			.append('feDropShadow')
			.attr('dx', 0)
			.attr('dy', 1)
			.attr('stdDeviation', 2)
			.attr('flood-opacity', 0.35);
	}

	private genderGlyphX(display: string): number {
		const nameWidth = truncate(display, 14).length * 6.5;
		return DENDRO_CONFIG.iconPadding * 2 + DENDRO_CONFIG.iconSize + 5 + nameWidth + 5;
	}

	hitTestNode(clientX: number, clientY: number): TreeNode | null {
		const t = this.currentTransform;
		const lx = (clientX - t.x) / t.k;
		const ly = (clientY - t.y) / t.k;
		for (const positioned of this.layoutNodes) {
			const nx = positioned.x;
			const ny = positioned.y - DENDRO_CONFIG.nodeHeight / 2;
			if (lx >= nx && lx <= nx + positioned.w && ly >= ny && ly <= ny + DENDRO_CONFIG.nodeHeight) {
				return positioned.node;
			}
		}
		return null;
	}

	setHovered(id: string | null): void {
		if (this.hoveredId === id) return;
		this.hoveredId = id;
		this.refreshNodeStyles();
		const node = id ? (this.layoutIndex.get(id)?.node ?? null) : null;
		this.callbacks.onHover?.(node as any, 0, 0);
	}

	setSelected(id: string | null): void {
		this.selectedId = id;
		this.refreshNodeStyles();
		const node = id ? (this.layoutIndex.get(id)?.node ?? null) : null;
		this.callbacks.onSelect?.(node as any);
	}

	/** Is `id` the focused node, or directly connected to it? */
	private isAdjacentToFocus(id: string): boolean {
		const focus = this.selectedId ?? this.hoveredId;
		if (!focus) return true;
		if (id === focus) return true;
		return this.layoutLinks.some(
			(l) =>
				(l.source.node.id === focus && l.target.node.id === id) ||
				(l.target.node.id === focus && l.source.node.id === id)
		);
	}

	private refreshNodeStyles(): void {
		if (!this.layoutNodes.length) return;
		const groups = this.nodesLayer.selectAll<SVGGElement, PositionedNode>('g.dendro-node');
		groups.attr('opacity', (d) => {
			if (!this.selectedId && !this.hoveredId) return 1;
			return this.isAdjacentToFocus(d.node.id) ? 1 : 0.35;
		});
		groups
			.select<SVGRectElement>('.dendro-card')
			.attr('fill', (d) => {
				if (d.node.id === this.selectedId) return this.colors.bgCardSelected;
				if (d.node.id === this.hoveredId) return this.colors.bgCardHover;
				if (d.node.isBred && !d.node.isTarget) return this.colors.bgCardBred;
				return this.colors.bgCard;
			})
			.attr('stroke', (d) => {
				if (d.node.isTarget) return this.colors.accentTarget;
				if (d.node.id === this.selectedId) return this.colors.accent;
				if (d.node.id === this.hoveredId) return this.colors.accentLight;
				return this.colors.line;
			})
			.attr('stroke-width', (d) => (d.node.isTarget ? 2.5 : 2));

		this.linksLayer
			.selectAll<SVGPathElement, PositionedLink>('path.dendro-link')
			.attr('stroke', (d) => {
				if (d.source.node.id === this.selectedId || d.target.node.id === this.selectedId) {
					return this.colors.linkHighlight;
				}
				return this.colors.link;
			})
			.attr('stroke-width', (d) => {
				if (d.source.node.id === this.selectedId || d.target.node.id === this.selectedId) {
					return 2.5;
				}
				return 1.5;
			})
			.attr('opacity', (d) => {
				if (!this.selectedId && !this.hoveredId) return 0.7;
				if (
					d.source.node.id === this.selectedId ||
					d.target.node.id === this.selectedId ||
					d.source.node.id === this.hoveredId ||
					d.target.node.id === this.hoveredId
				) {
					return 1;
				}
				return 0.2;
			});
	}

	getNode(id: string): TreeNode | null {
		return this.layoutIndex.get(id)?.node ?? null;
	}

	fit(): void {
		if (!this.layoutNodes.length) return;
		const xs = this.layoutNodes.map((n) => n.x);
		const ys = this.layoutNodes.map((n) => n.y);
		const minX = Math.min(...xs);
		const maxX = Math.max(...xs) + Math.max(...this.layoutNodes.map((n) => n.w));
		const minY = Math.min(...ys) - DENDRO_CONFIG.nodeHeight / 2;
		const maxY = Math.max(...ys) + DENDRO_CONFIG.nodeHeight / 2;
		const treeW = Math.max(maxX - minX, 1);
		const treeH = Math.max(maxY - minY, 1);

		const svgEl = this.svg.node();
		if (!svgEl) return;
		const w = svgEl.clientWidth;
		const h = svgEl.clientHeight;
		if (!w || !h) return;

		const scale = Math.min(
			(w - DENDRO_CONFIG.fitMargin * 2) / treeW,
			(h - DENDRO_CONFIG.fitMargin * 2) / treeH,
			DENDRO_CONFIG.zoom.max
		);
		const clamped = Math.max(scale, DENDRO_CONFIG.zoom.min);
		const tx = (w - treeW * clamped) / 2 - minX * clamped;
		const ty = (h - treeH * clamped) / 2 - minY * clamped;
		const t = zoomIdentity.translate(tx, ty).scale(clamped);
		this.svg
			.transition()
			.duration(DENDRO_CONFIG.animation.durationMs)
			.call(this.zoomBehavior.transform, t);
	}

	zoomBy(factor: number): void {
		this.zoomBehavior.scaleBy(
			this.svg.transition().duration(DENDRO_CONFIG.animation.durationMs),
			factor
		);
	}

	reset(): void {
		this.fit();
	}

	getZoomScale(): number {
		return zoomTransform(this.svg.node()!).k;
	}

	destroy(): void {
		this.svg.selectAll('*').remove();
		this.svg.on('.zoom', null);
	}
}

function truncate(s: string, max: number): string {
	return s.length <= max ? s : s.slice(0, max - 1) + '…';
}

/**
 * Shrink `el`'s text (with a trailing ellipsis) until its measured width fits
 * `maxWidth`. Width is measured, so CJK/wide glyphs respect the same limit as
 * ASCII — a fixed char-count truncation cannot. No-op when already fits.
 * Requires the element to be laid out; callers guard with try/catch before
 * layout stabilizes.
 */
function fitWidth(el: SVGTextElement, maxWidth: number): void {
	const full = el.textContent ?? '';
	el.textContent = full;
	if (el.getComputedTextLength() <= maxWidth) return;
	let best = '…';
	let lo = 1;
	let hi = full.length;
	while (lo <= hi) {
		const mid = (lo + hi) >> 1;
		el.textContent = full.slice(0, mid) + '…';
		if (el.getComputedTextLength() <= maxWidth) {
			best = el.textContent!;
			lo = mid + 1;
		} else {
			hi = mid - 1;
		}
	}
	el.textContent = best;
}

function genderGlyph(gender?: string | null): string {
	switch (gender) {
		case 'Male':
			return '♂';
		case 'Female':
			return '♀';
		default:
			return '';
	}
}

function genderColor(colors: DendroColors, gender?: string | null): string {
	switch (gender) {
		case 'Male':
			return colors.male;
		case 'Female':
			return colors.female;
		default:
			return colors.wildcard;
	}
}

function cssEscape(s: string): string {
	if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') {
		return CSS.escape(s);
	}
	return s.replace(/[^a-zA-Z0-9_-]/g, '_');
}

void transition;
