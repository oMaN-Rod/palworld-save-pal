import { readFile, readdir, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Cloudflare Workers static assets: 20,000 files on the free plan, 100,000 on
// paid, 25 MiB per file. 18,000 leaves headroom before the free-tier wall.
export const LIMITS = { maxFiles: 18000, maxBytes: 25 * 1024 * 1024 };

/**
 * Routes that must contain full SEO markup after a build. Locale roots are
 * emitted as `<slug>.html`, not `<slug>/index.html`.
 */
const SAMPLE_PAGES = [
	'index.html',
	'map.html',
	'wiki.html',
	'breeding.html',
	'about.html',
	'wiki/pals/sheepball.html',
	'wiki/items.html',
	'es.html',
	'fr/map.html'
];

export function checkFileBudget(files, limits = LIMITS) {
	const errors = [];
	if (files.length > limits.maxFiles) {
		errors.push(
			`Asset budget exceeded: ${files.length} files, limit ${limits.maxFiles}. ` +
				'Cloudflare allows 20,000 on the free plan.'
		);
	}
	for (const file of files) {
		if (file.size > limits.maxBytes) {
			errors.push(
				`File too large: ${file.path} is ${(file.size / 1024 / 1024).toFixed(1)} MiB, limit 25 MiB.`
			);
		}
	}
	return { errors, fileCount: files.length };
}

export function checkPageMarkup(html) {
	const errors = [];
	if (html.includes('%lang%')) errors.push('unsubstituted %lang% placeholder');
	if (html.includes('%sveltekit')) errors.push('unsubstituted sveltekit placeholder');
	if (!/<title>[^<]+<\/title>/.test(html)) errors.push('missing <title>');
	if (!/<meta name="description" content="[^"]+"/.test(html)) {
		errors.push('missing meta description');
	}
	if (!/<link rel="canonical"/.test(html)) errors.push('missing canonical link');
	if (!/<h1[\s>]/.test(html)) errors.push('missing <h1>');
	return errors;
}

async function walk(dir, base = dir) {
	const out = [];
	for (const entry of await readdir(dir, { withFileTypes: true })) {
		const full = join(dir, entry.name);
		if (entry.isDirectory()) {
			out.push(...(await walk(full, base)));
		} else {
			out.push({
				path: full.slice(base.length + 1).replaceAll('\\', '/'),
				size: (await stat(full)).size
			});
		}
	}
	return out;
}

async function main() {
	const buildDir = resolve(dirname(fileURLToPath(import.meta.url)), '../../ui_build');
	const files = await walk(buildDir);
	const { errors, fileCount } = checkFileBudget(files);

	for (const page of SAMPLE_PAGES) {
		let html;
		try {
			html = await readFile(join(buildDir, page), 'utf8');
		} catch {
			errors.push(`Expected page missing from build: ${page}`);
			continue;
		}
		for (const problem of checkPageMarkup(html)) {
			errors.push(`${page}: ${problem}`);
		}
	}

	const totalMb = (files.reduce((sum, file) => sum + file.size, 0) / 1024 / 1024).toFixed(0);
	console.log(`Build output: ${fileCount} files, ${totalMb} MB.`);

	if (errors.length > 0) {
		console.error(`\n${errors.length} problem(s):`);
		for (const error of errors) console.error(`  - ${error}`);
		process.exit(1);
	}
	console.log('Build output checks passed.');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	await main();
}
