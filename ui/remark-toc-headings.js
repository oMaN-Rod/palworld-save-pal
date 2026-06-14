import { visit } from 'unist-util-visit';

/**
 * Remark plugin to add data-toc attribute to headings marked with {.toc}
 *
 * Usage in markdown:
 * ## My Heading {.toc}
 * ### Another Section {.toc}
 * ## Regular Heading (will not appear in TOC)
 */
export function remarkTocHeadings() {
	return function transformer(tree) {
		visit(tree, 'heading', (node) => {
			if (node.depth < 2 || node.depth > 4) return;

			const tocMarkerRegex = /\s*\{[#.]toc\}\s*$/;

			for (let i = 0; i < node.children.length; i++) {
				const child = node.children[i];
				if (child.type === 'text') {
					if (tocMarkerRegex.test(child.value)) {
						child.value = child.value.replace(tocMarkerRegex, '').trim();
						node.data = node.data || {};
						node.data.hProperties = node.data.hProperties || {};
						node.data.hProperties['data-toc'] = true;
						break;
					}
				}
			}
		});

		return tree;
	};
}