/**
 * Hand-curated icon remap: lucide-collection names -> Tabler/Phosphor picks.
 * Every entry is an intentional visual choice; `candidates` are acceptable
 * alternates (first existing one wins) for names Tabler may not carry.
 *
 * Run from ui/:  node scripts/icon-remap.mjs [--write]
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';

const WRITE = process.argv.includes('--write');
const UI_ROOT = new URL('..', import.meta.url).pathname;

const collections = {
	tabler: JSON.parse(
		readFileSync(join(UI_ROOT, 'node_modules/@iconify-json/tabler/icons.json'), 'utf8')
	),
	ph: JSON.parse(readFileSync(join(UI_ROOT, 'node_modules/@iconify-json/ph/icons.json'), 'utf8'))
};
const has = (coll, name) => !!(collections[coll].icons[name] ?? collections[coll].aliases?.[name]);

/** lucide name -> primary pick + accepted alternates, in preference order. */
const MAP = {
	// actions & controls
	x: ['tabler:x'],
	play: ['tabler:player-play'],
	hand: ['tabler:hand-grab', 'ph:hand-pointing'],
	trees: ['tabler:trees'],
	'layout-list': ['tabler:layout-list'],
	history: ['tabler:history'],
	repeat: ['tabler:repeat'],
	'list-checks': ['tabler:list-check', 'tabler:checklist'],
	egg: ['tabler:egg', 'ph:egg'],
	flame: ['tabler:flame'],
	hammer: ['tabler:hammer', 'tabler:gavel', 'tabler:tool'],
	save: ['tabler:device-floppy'],
	plus: ['tabler:plus'],
	minus: ['tabler:minus'],
	check: ['tabler:check'],
	'check-check': ['tabler:checks'],
	copy: ['tabler:copy'],
	'square-pen': ['tabler:edit'],
	pencil: ['tabler:pencil'],
	delete: ['tabler:backspace'],
	'replace-all': ['tabler:arrows-diff', 'tabler:transfer', 'tabler:arrows-horizontal'],
	upload: ['tabler:upload'],
	download: ['tabler:download'],
	share: ['tabler:share', 'tabler:share-2', 'tabler:share-3'],
	send: ['tabler:send'],
	'external-link': ['tabler:external-link'],
	refresh: ['tabler:repeat'],
	'refresh-cw': ['tabler:refresh'],
	'refresh-ccw': ['tabler:rotate'],
	'rotate-ccw': ['tabler:rotate-counter-clockwise', 'tabler:rotate'],
	'timer-reset': ['tabler:history', 'tabler:rotate-2', 'tabler:reload'],
	maximize: ['tabler:arrows-maximize'],
	'maximize-2': ['tabler:maximize'],
	'circle-fading-plus': ['tabler:circle-plus'],
	'package-plus': ['tabler:package-import', 'tabler:file-plus'],
	'archive-restore': ['tabler:archive-off', 'tabler:archive'],
	'list-x': ['tabler:list-x', 'tabler:playlist-x', 'tabler:eraser'],

	// data & files
	trash: ['tabler:trash'],
	'trash-2': ['tabler:trash-x'],
	folder: ['tabler:folder'],
	'folder-open': ['tabler:folder-open'],
	'folder-archive': ['tabler:archive'],
	'file-text': ['tabler:file-text'],
	'file-json': ['tabler:file-type-json', 'tabler:braces', 'tabler:file-code'],
	'file-box': ['tabler:file-database', 'tabler:database', 'tabler:file'],
	'file-archive': ['tabler:file-zip', 'tabler:file-download', 'tabler:file'],
	database: ['tabler:database'],
	'hard-drive': ['tabler:device-hdd', 'ph:hard-drives', 'ph:hard-drive'],
	archive: ['tabler:archive'],
	package: ['tabler:package'],
	scroll: ['tabler:scroll', 'ph:scroll', 'tabler:file-text'],

	// navigation & layout
	map: ['tabler:map'],
	'land-plot': ['tabler:fence', 'tabler:map-2', 'tabler:home-2'],
	'map-pin': ['tabler:map-pin'],
	navigation: ['tabler:navigation', 'tabler:navigation-2'],
	route: ['tabler:route'],
	globe: ['tabler:world'],
	'globe-2': ['tabler:world-longitude', 'tabler:world'],
	compass: ['tabler:compass'],
	'layout-grid': ['tabler:layout-grid'],
	'grid-3x3': ['tabler:grid-3x3', 'tabler:grid-4x4', 'tabler:layout-grid'],
	'gallery-vertical-end': ['tabler:layout-list', 'tabler:layout-bottombar', 'tabler:list'],
	list: ['tabler:list'],
	layers: ['tabler:stack-2', 'tabler:stack'],
	'panel-left': ['tabler:layout-sidebar-left', 'tabler:layout-sidebar'],
	'panel-left-close': [
		'tabler:layout-sidebar-left-collapse',
		'tabler:layout-sidebar-left',
		'tabler:layout-sidebar'
	],
	'chevrons-left-right': ['tabler:arrows-move-horizontal', 'tabler:arrows-horizontal'],
	'chevrons-up-down': ['tabler:arrows-move-vertical', 'tabler:arrows-vertical'],
	'chevron-down': ['tabler:chevron-down'],
	'chevron-up': ['tabler:chevron-up'],
	'chevron-left': ['tabler:chevron-left'],
	'chevron-right': ['tabler:chevron-right'],
	'chevrons-left': ['tabler:chevrons-left'],
	'chevrons-right': ['tabler:chevrons-right'],
	'arrow-left': ['tabler:arrow-left'],
	'arrow-right': ['tabler:arrow-right'],
	'arrow-right-left': ['tabler:transfer', 'tabler:arrows-horizontal', 'tabler:arrow-left-right'],

	// sort indicators
	'arrow-down-a-z': ['tabler:sort-ascending-letters'],
	'arrow-down-z-a': ['tabler:sort-descending-letters'],
	'arrow-up-a-z': ['ph:sort-ascending'],
	'arrow-down-0-1': ['tabler:sort-ascending-numbers'],
	'arrow-down-1-0': ['tabler:sort-descending-numbers'],
	'arrow-up-0-1': ['ph:sort-ascending'],
	'arrow-down-wide-narrow': ['tabler:arrows-sort', 'tabler:sort-ascending'],
	'arrow-down-narrow-wide': ['tabler:arrows-sort', 'tabler:sort-ascending'],

	// people, guilds & servers
	user: ['tabler:user'],
	users: ['tabler:users'],
	'building-2': ['tabler:building-community', 'tabler:building'],
	building: ['tabler:building'],
	castle: ['tabler:building-castle', 'tabler:building-arch'],
	server: ['tabler:server'],
	network: ['tabler:topology-star-3', 'tabler:topology-star', 'tabler:sitemap'],
	monitor: ['tabler:device-desktop'],
	'monitor-down': ['tabler:device-desktop-down', 'tabler:device-desktop-analytics'],
	github: ['tabler:brand-github'],
	'gamepad-2': ['tabler:device-gamepad-2', 'tabler:device-gamepad'],

	// security & status
	lock: ['tabler:lock'],
	'lock-open': ['tabler:lock-open'],
	'lock-open-alt': ['tabler:lock-open'],
	key: ['tabler:key'],
	shield: ['tabler:shield'],
	'shield-check': ['tabler:shield-check'],
	'shield-x': ['tabler:shield-x'],
	eye: ['tabler:eye'],
	'eye-off': ['tabler:eye-off'],
	'eye-closed': ['tabler:eye-closed', 'tabler:eye-off'],
	ban: ['tabler:ban'],
	'octagon-x': ['tabler:octagon-x', 'tabler:alert-octagon', 'tabler:ban'],
	info: ['tabler:info-circle'],
	circle: ['tabler:circle'],
	'circle-alert': ['tabler:alert-circle'],
	'triangle-alert': ['tabler:alert-triangle'],
	'circle-question-mark': ['tabler:help-circle', 'tabler:circle-help'],
	'circle-check': ['tabler:circle-check'],
	'circle-check-big': ['tabler:circle-check'],
	'circle-x': ['tabler:circle-x'],
	'clock-alert': ['tabler:clock-exclamation', 'tabler:alarm', 'tabler:clock-pause'],
	clock: ['tabler:clock'],
	'help-circle': ['tabler:help-circle'],

	// editor & tools
	search: ['tabler:search'],
	'search-x': ['tabler:search-off', 'tabler:zoom-cancel'],
	funnel: ['tabler:filter'],
	settings: ['tabler:settings'],
	'settings-2': ['tabler:adjustments', 'tabler:settings-2'],
	adjustments: ['tabler:adjustments'],
	terminal: ['tabler:terminal-2', 'tabler:terminal'],
	code: ['tabler:code'],
	cpu: ['tabler:cpu'],
	'memory-stick': ['ph:memory', 'tabler:cpu-2', 'tabler:device-sd-card'],
	'text-wrap': ['tabler:text-wrap', 'tabler:arrow-wrap'],
	'word-wrap': ['tabler:text-wrap'],
	wrench: ['tabler:tool', 'tabler:tools'],
	'paint-bucket': ['tabler:paint'],
	palette: ['tabler:palette'],
	ruler: ['tabler:ruler-2', 'tabler:ruler'],
	'chart-column': ['tabler:chart-bar', 'tabler:chart-histogram', 'tabler:chart-arrows'],
	'trending-up': ['tabler:trending-up'],
	activity: ['tabler:activity'],
	target: ['tabler:target'],
	'grip-vertical': ['tabler:grip-vertical'],
	'grip-horizontal': ['tabler:grip-horizontal'],

	// creatures, combat & game flavor
	'paw-print': ['tabler:pawprint', 'ph:paw-print'],
	venus: ['ph:gender-female', 'ph:venus'],
	mars: ['ph:gender-male', 'ph:mars'],
	'biceps-flexed': ['ph:hand-fist', 'ph:barbell', 'tabler:dumbbell'],
	heart: ['tabler:heart'],
	skull: ['tabler:skull'],
	sword: ['tabler:sword'],
	swords: ['tabler:swords'],
	trophy: ['tabler:trophy'],
	award: ['tabler:award'],
	bomb: ['tabler:bomb', 'ph:bomb'],
	rocket: ['tabler:rocket'],
	'party-popper': ['ph:confetti', 'tabler:party-popper'],
	dices: ['tabler:dice-5', 'tabler:dice-3'],
	gamepad: ['tabler:device-gamepad'],
	puzzle: ['tabler:puzzle'],
	sparkles: ['tabler:sparkles'],
	gem: ['tabler:diamond'],
	crown: ['tabler:crown'],
	apple: ['tabler:apple'],
	pizza: ['tabler:pizza'],
	ambulance: ['tabler:ambulance', 'ph:ambulance', 'tabler:first-aid'],
	bandage: ['tabler:bandage', 'ph:bandage', 'tabler:first-aid-kit'],
	brain: ['tabler:brain', 'ph:brain'],
	bug: ['tabler:bug'],
	'flask-conical': ['tabler:flask', 'tabler:flask-2'],
	'git-merge': ['tabler:git-merge'],
	'git-fork': ['tabler:git-fork'],
	spline: ['tabler:spline', 'tabler:vector-spline'],
	orbit: ['tabler:atom', 'tabler:spiral'],
	'columns-3': ['tabler:columns-3', 'tabler:layout-columns'],
	cuboid: ['ph:cube', 'tabler:3d-cube-sphere'],
	star: ['tabler:star'],
	zap: ['tabler:bolt'],
	'trending-down': ['tabler:trending-down'],
	thermometer: ['tabler:temperature', 'tabler:temperature-high'],
	'notebook-pen': ['tabler:notebook', 'tabler:notes'],
	'sticky-note': ['tabler:note', 'tabler:notes', 'tabler:clipboard'],
	'book-open': ['tabler:book', 'tabler:book-2'],
	'file-heart': ['tabler:file-heart', 'tabler:file-like'],
	house: ['tabler:home', 'tabler:home-2'],
	'layout-dashboard': ['tabler:layout-dashboard'],
	square: ['tabler:square'],
	box: ['tabler:box'],
	'heart-crack': ['tabler:heart-broken'],
	lang: ['tabler:language'],
	languages: ['tabler:language', 'tabler:language-hiragana'],
	'monitor-smartphone': ['tabler:devices', 'tabler:device-mobile'],
	gift: ['tabler:gift'],
	'code-xml': ['tabler:code'],
	'mouse-pointer-click': ['tabler:hand-click', 'tabler:cursor'],
	boxes: ['tabler:boxes', 'tabler:box-multiple'],
	'lang-graph': ['tabler:language'],
	bookmark: ['tabler:bookmark'],
	blocks: ['tabler:blocks', 'tabler:components'],
	calendar: ['tabler:calendar'],
	'circle-dashed': ['tabler:circle-dashed'],
	'loader-circle': ['tabler:loader-2', 'tabler:loader'],
	'folder-dot': ['tabler:folder-share', 'tabler:folder'],
	'folder-plus': ['tabler:folder-plus'],
	tag: ['tabler:tag'],
	tags: ['tabler:tags'],
	hash: ['tabler:hash'],
	'lock-password': ['tabler:lock-password']
};

// Resolve each entry to the first candidate that exists.
const resolved = {};
const missing = [];
for (const [from, candidates] of Object.entries(MAP)) {
	const pick = candidates.find((c) => has(c.split(':')[0], c.split(':')[1]));
	if (pick) resolved[from] = pick;
	else missing.push({ from, candidates });
}

// Ensure every lucide name actually used in src/ has a mapping.
const files = [];
(function walk(dir) {
	for (const e of readdirSync(dir)) {
		const p = join(dir, e);
		if (statSync(p).isDirectory()) walk(p);
		else if (/\.(svelte|ts)$/.test(e)) files.push(p);
	}
})(join(UI_ROOT, 'src'));

const used = new Set();
for (const f of files) {
	const s = readFileSync(f, 'utf8');
	for (const m of s.matchAll(/lucide:([a-z0-9-]+)/g)) used.add(m[1]);
}
const unmapped = [...used].filter((u) => !resolved[u]);
const unusedMappings = Object.keys(resolved).filter((k) => !used.has(k));

console.log(`used lucide names: ${used.size}, mappings resolved: ${Object.keys(resolved).length}`);
if (missing.length) console.log('MAP ENTRIES WITH NO EXISTING CANDIDATE:', missing);
if (unmapped.length) console.log('USED BUT UNMAPPED:', unmapped);
if (unusedMappings.length) console.log('(unused mapping keys, harmless):', unusedMappings);

if (WRITE && !unmapped.length && !missing.length) {
	let total = 0;
	for (const f of files) {
		let s = readFileSync(f, 'utf8');
		for (const [from, to] of Object.entries(resolved)) {
			s = s.split(`"lucide:${from}"`).join(`"${to}"`);
			s = s.split(`'lucide:${from}'`).join(`'${to}'`);
		}
		const before = readFileSync(f, 'utf8');
		if (s !== before) {
			const diffCount = (before.match(/lucide:/g) || []).length;
			total += diffCount;
			writeFileSync(f, s);
			console.log(`${relative(UI_ROOT, f)}: ${diffCount} replacements`);
		}
	}
	console.log(`TOTAL: ${total}`);
}
