import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	LOCALIZED_PATHS,
	SITE_ORIGIN,
	hrefLanguageTags,
	localizedPath,
	siteLocales
} from '../src/lib/i18n/routingConfig.js';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const staticDir = resolve(scriptDir, '../static');
const dataDir = resolve(scriptDir, '../../data/json');

/**
 * Wiki categories with their key source and whether keys carry a `X::` prefix.
 * `work-suitability` has no raw data file — its keys are synthesized in the app
 * — so it reads its key set from the l10n catalogue instead.
 */
const WIKI_CATEGORIES = [
	{ id: 'pals', file: 'pals.json', stripPrefix: false },
	{ id: 'items', file: 'items.json', stripPrefix: true },
	{ id: 'buildings', file: 'buildings.json', stripPrefix: true },
	{ id: 'active-skills', file: 'active_skills.json', stripPrefix: true },
	{ id: 'passive-skills', file: 'passive_skills.json', stripPrefix: true },
	{ id: 'technologies', file: 'technologies.json', stripPrefix: true },
	{ id: 'elements', file: 'elements.json', stripPrefix: true },
	{ id: 'work-suitability', file: 'l10n/en/work_suitability.json', stripPrefix: true }
];

const GUIDES = ['getting-started', 'save-management', 'server-setup'];

/**
 * Public pages outside `LOCALIZED_PATHS`: one English URL each, no alternates.
 * The raw editor is a tool page, not a hub -- prefixing it into all 16 locales
 * would ship 16 copies of an interface that is English either way.
 */
const ENGLISH_ONLY_PAGES = ['/editor'];

export function xmlEscape(value) {
	return value
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;')
		.replaceAll("'", '&apos;');
}

// Mirrors toSlug in src/lib/utils/wikiSlug.ts. Duplicated because this script
// runs in plain Node, without the TypeScript pipeline or Vite path aliases.
export function toSlug(key) {
	return key
		.replace(/([a-z0-9])([A-Z])/g, '$1-$2')
		.replace(/[_\s]+/g, '-')
		.toLowerCase()
		.replace(/[^a-z0-9-]+/g, '-')
		.replace(/-+/g, '-')
		.replace(/^-|-$/g, '');
}

export function stripKeyPrefix(key) {
	const index = key.lastIndexOf('::');
	return index === -1 ? key : key.slice(index + 2);
}

export function buildUrlEntry(pathname, { localized, priority = '0.6', changefreq = 'weekly' }) {
	const loc = `${SITE_ORIGIN}${pathname}`;
	const lines = ['\t<url>', `\t\t<loc>${xmlEscape(loc)}</loc>`];
	if (localized) {
		for (const locale of siteLocales) {
			const href = `${SITE_ORIGIN}${localizedPath(pathname, locale)}`;
			lines.push(
				`\t\t<xhtml:link rel="alternate" hreflang="${hrefLanguageTags[locale]}" href="${xmlEscape(href)}" />`
			);
		}
		const fallback = `${SITE_ORIGIN}${localizedPath(pathname, 'en')}`;
		lines.push(
			`\t\t<xhtml:link rel="alternate" hreflang="x-default" href="${xmlEscape(fallback)}" />`
		);
	}
	lines.push(
		`\t\t<changefreq>${changefreq}</changefreq>`,
		`\t\t<priority>${priority}</priority>`,
		'\t</url>'
	);
	return lines.join('\n');
}

export function buildUrlset(entries) {
	return [
		'<?xml version="1.0" encoding="UTF-8"?>',
		'<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">',
		...entries,
		'</urlset>',
		''
	].join('\n');
}

export function buildSitemapIndex(paths) {
	return [
		'<?xml version="1.0" encoding="UTF-8"?>',
		'<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
		...paths.map(
			(path) => `\t<sitemap>\n\t\t<loc>${xmlEscape(`${SITE_ORIGIN}/${path}`)}</loc>\n\t</sitemap>`
		),
		'</sitemapindex>',
		''
	].join('\n');
}

async function readJson(file) {
	return JSON.parse(await readFile(resolve(dataDir, file), 'utf8'));
}

function isDisabled(record) {
	if (!record || typeof record !== 'object') return false;
	if (record.disabled === true) return true;
	return !!record.details && record.details.disabled === true;
}

async function main() {
	await mkdir(resolve(staticDir, 'sitemaps'), { recursive: true });
	const children = [];

	const hubEntries = LOCALIZED_PATHS.flatMap((pathname) =>
		siteLocales.map((locale) =>
			buildUrlEntry(localizedPath(pathname, locale), {
				localized: true,
				priority: pathname === '/' ? '1.0' : '0.8'
			})
		)
	);
	const englishOnlyEntries = ENGLISH_ONLY_PAGES.map((pathname) =>
		buildUrlEntry(pathname, { localized: false, priority: '0.7' })
	);
	await writeFile(
		resolve(staticDir, 'sitemaps/pages.xml'),
		buildUrlset([...hubEntries, ...englishOnlyEntries]),
		'utf8'
	);
	children.push('sitemaps/pages.xml');

	let wikiCount = 0;
	for (const category of WIKI_CATEGORIES) {
		const json = await readJson(category.file);
		const entries = Object.entries(json)
			.filter(([, record]) => !isDisabled(record))
			.map(([key]) => {
				const slug = toSlug(category.stripPrefix ? stripKeyPrefix(key) : key);
				return buildUrlEntry(`/wiki/${category.id}/${slug}`, {
					localized: false,
					priority: '0.5'
				});
			});
		const indexEntry = buildUrlEntry(`/wiki/${category.id}`, {
			localized: false,
			priority: '0.7'
		});
		const file = `sitemaps/wiki-${category.id}.xml`;
		await writeFile(resolve(staticDir, file), buildUrlset([indexEntry, ...entries]), 'utf8');
		children.push(file);
		wikiCount += entries.length;
	}

	const guides = GUIDES.map((slug) =>
		buildUrlEntry(`/docs/guides/${slug}`, { localized: false, priority: '0.7' })
	);
	await writeFile(resolve(staticDir, 'sitemaps/guides.xml'), buildUrlset(guides), 'utf8');
	children.push('sitemaps/guides.xml');

	await writeFile(resolve(staticDir, 'sitemap.xml'), buildSitemapIndex(children), 'utf8');
	console.log(
		`Sitemap: ${hubEntries.length} hub URLs, ${englishOnlyEntries.length} English-only URLs, ` +
			`${wikiCount} wiki URLs, ${guides.length} guides across ${children.length} child sitemaps.`
	);
}

// pathToFileURL rather than string-concatenating `file://`: on Windows the
// latter yields file://O:/... while import.meta.url is file:///O:/...
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	await main();
}
