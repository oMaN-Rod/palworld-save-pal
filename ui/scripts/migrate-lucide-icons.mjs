/**
 * One-shot migration: rewrites lucide-svelte icon usage to the bundled
 * Iconify `Icon` wrapper (`$lib/components/ui/icons/Icon.svelte`).
 *
 *   node scripts/migrate-lucide-icons.mjs [--write]
 *
 * Without --write it only reports what it would change. Handles:
 *   - `import X from '@lucide/svelte/icons/name'` and root-form imports
 *   - tag usage `<X ...>` / `</X>`
 *   - value usage `={X}`, `=> X`, ternary branches, `icon: X` object keys
 *   - type usage `typeof X` -> `string`
 * Files with leftovers (aliases that could not be rewritten safely) are
 * listed for manual follow-up.
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const WRITE = process.argv.includes('--write');
const UI_ROOT = new URL('..', import.meta.url).pathname;
const SRC = join(UI_ROOT, 'src');
const ICON_IMPORT = "import Icon from '$lib/components/ui/icons/Icon.svelte';";

const files = [];
(function walk(dir) {
	for (const entry of readdirSync(dir)) {
		const p = join(dir, entry);
		const st = statSync(p);
		if (st.isDirectory()) walk(p);
		else if (/\.(svelte|ts|js|mjs)$/.test(entry)) files.push(p);
	}
})(SRC);

const importLine =
	/^\t?import\s+(?:([A-Z]\w*)\s+from\s+'@lucide\/svelte\/icons\/([a-z0-9-]+)'|\{([^}]+)\}\s+from\s+'@lucide\/svelte');\s*$/gm;
const pascalToKebab = (s) => s.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();

let totalTags = 0;
let totalValues = 0;
const leftoverReport = [];
const valueReport = [];

for (const file of files) {
	const src = readFileSync(file, 'utf8');
	if (!src.includes('@lucide')) continue;
	const rel = relative(UI_ROOT, file);

	const lines = src.split('\n');
	const aliasMap = new Map(); // alias -> lucide icon name
	const importLineIdxs = [];
	let cursor;
	importLine.lastIndex = 0;
	while ((cursor = importLine.exec(src))) {
		const line = cursor[0].replace(/\n$/, '');
		const idx = lines.indexOf(line);
		if (idx === -1 || importLineIdxs.includes(idx)) continue;
		importLineIdxs.push(idx);
		if (cursor[1]) {
			aliasMap.set(cursor[1], cursor[2]);
		} else {
			for (const part of cursor[3].split(',')) {
				const alias = part.trim();
				if (alias) aliasMap.set(alias, pascalToKebab(alias));
			}
		}
	}
	if (aliasMap.size === 0) continue;

	let body = lines
		.map((line, i) => (importLineIdxs.includes(i) ? null : line))
		.filter((l) => l !== null)
		.join('\n');

	// drop blank line runs left behind at removal points (cosmetic; prettier will finish)
	let tags = 0;
	let values = 0;
	const sorted = [...aliasMap.keys()].sort((a, b) => b.length - a.length);
	for (const alias of sorted) {
		const name = aliasMap.get(alias);
		const esc = alias.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

		body = body.replace(new RegExp(`<(${esc})(?=[\\s/>])`, 'g'), () => {
			tags++;
			return `<Icon icon="lucide:${name}"`;
		});
		body = body.replace(new RegExp(`</${esc}\\s*>`, 'g'), () => {
			tags++;
			return '</Icon>';
		});
		for (const pattern of [
			[new RegExp(`=\\{\\s*${esc}\\s*\\}`, 'g'), 'attr'],
			[new RegExp(`(=>\\s*)${esc}(?![\\w$])`, 'g'), 'arrow'],
			[new RegExp(`(\\?\\s*)${esc}(?=\\s*:)`, 'g'), 'ternary-true'],
			[new RegExp(`(\\?[^:?{}]*:\\s*)${esc}(?![\\w$])`, 'g'), 'ternary-false'],
			[new RegExp(`(\\bicon:\\s*)${esc}(?=[,}\\s])`, 'g'), 'object-key']
		]) {
			body = body.replace(pattern[0], (...args) => {
				values++;
				const match = args[0];
				const g1 = typeof args[1] === 'string' ? args[1] : '';
				valueReport.push(`${rel}: [${pattern[1]}] ${match.trim()}`);
				return `${g1}"lucide:${name}"`;
			});
		}
		body = body.replace(new RegExp(`typeof\\s+${esc}\\b`, 'g'), 'string');
	}

	if (tags > 0 && !body.includes(ICON_IMPORT)) {
		// insert where the first lucide import used to live: after the first import line
		const firstImport = body.match(/^import .*$/m);
		if (firstImport) {
			body = body.replace(firstImport[0], `${firstImport[0]}\n${ICON_IMPORT}`);
		} else {
			body = `${ICON_IMPORT}\n${body}`;
		}
	}

	const remaining = [...aliasMap.keys()].filter((alias) => new RegExp(`\\b${alias}\\b`).test(body));
	if (remaining.length) leftoverReport.push(`${rel}: ${remaining.join(', ')}`);

	if (tags || values) {
		totalTags += tags;
		totalValues += values;
		if (WRITE) writeFileSync(file, body);
		console.log(`${rel}: ${tags} tags, ${values} values`);
	}
}

console.log(
	`\n${WRITE ? 'Wrote' : 'Would write'}: ${totalTags} tag + ${totalValues} value replacements`
);
if (leftoverReport.length) {
	console.log('\nMANUAL FOLLOW-UP (aliases still referenced):');
	for (const l of leftoverReport) console.log('  ' + l);
}
if (valueReport.length) {
	console.log('\nValue-form replacements for review:');
	for (const l of valueReport.slice(0, 80)) console.log('  ' + l);
}
