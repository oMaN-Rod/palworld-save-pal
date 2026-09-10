import {
	activeSkillsData,
	buildingsData,
	elementsData,
	itemsData,
	palsData,
	passiveSkillsData,
	technologiesData,
	workSuitabilityData,
	WORK_SUITABILITY_KEYS
} from '$lib/data';
import { ASSET_DATA_PATH } from '$lib/constants';
import { assetLoader } from './assetLoader';
import { rarityGradientClass, skillFilter } from './colors';
import { suitabilityImageMap } from './pals';
import type { ElementType, PalData, WorkSuitability } from '$types';
import type { WikiCategory } from './wikiCategories';

export type WikiField = {
	label: string;
	value: (record: Record<string, unknown>) => string | number | null;
};

export type WikiIcon = { src: string; color?: string; filter?: string };

export type WikiRelated = {
	category: WikiCategory;
	key: string;
	label: string;
	sublabel?: string;
	icon?: WikiIcon | null;
	missing?: boolean;
};

export type WikiExtra = { src: string; label: string };

export type WikiDescriptor = {
	loadJson: () => Promise<Record<string, unknown>>;
	runtime: () => Record<string, unknown>;
	displayName: (key: string, record: Record<string, unknown> | undefined) => string;
	fields: WikiField[];
	description?: (record: Record<string, unknown>) => string | null;
	icon?: (key: string, record: Record<string, unknown>) => WikiIcon | null;
	cardMeta?: (key: string, record: Record<string, unknown>) => string | null;
	iconBackground?: (key: string, record: Record<string, unknown>) => string | null;
	extras?: (key: string, record: Record<string, unknown>) => WikiExtra[];
	related?: (key: string, record: Record<string, unknown>) => WikiRelated[];
};

function get(record: Record<string, unknown>, ...path: string[]): unknown {
	let current: unknown = record;
	for (const segment of path) {
		if (current === null || typeof current !== 'object') return undefined;
		current = (current as Record<string, unknown>)[segment];
	}
	return current;
}

function field(record: Record<string, unknown>, ...path: string[]): string | number | null {
	const value = get(record, ...path);
	if (value === undefined || value === null) return null;
	if (typeof value === 'string' || typeof value === 'number') return value;
	if (typeof value === 'boolean') return value ? 'Yes' : 'No';
	if (Array.isArray(value)) {
		if (value.length === 0) return null;
		return value
			.map((item) =>
				item && typeof item === 'object' && 'id' in item
					? `${(item as Record<string, unknown>).id}${
							'count' in item ? ` x${(item as Record<string, unknown>).count}` : ''
						}`
					: item && typeof item === 'object' && 'type' in item
						? String((item as Record<string, unknown>).type)
						: String(item)
			)
			.join(', ');
	}
	return null;
}

function nameOr(key: string, record: Record<string, unknown> | undefined, ...path: string[]): string {
	if (!record) return key;
	const value = path.length > 0 ? get(record, ...path) : record.localized_name;
	return typeof value === 'string' && value.length > 0 ? value : key;
}

function descriptionField(...path: string[]) {
	return (record: Record<string, unknown>): string | null => {
		const value = get(record, ...path);
		return typeof value === 'string' && value.length > 0 ? value : null;
	};
}

function assetIcon(iconId: unknown, color?: unknown): WikiIcon | null {
	if (typeof iconId !== 'string' || iconId.length === 0) return null;
	const src = assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${iconId}.webp`);
	if (!src) return null;
	return typeof color === 'string' && color.length > 0 ? { src, color } : { src };
}

function iconField(...path: string[]) {
	return (_key: string, record: Record<string, unknown>): WikiIcon | null =>
		assetIcon(get(record, ...path));
}

export { isHiddenRecord } from './wikiSlug';

function palIcon(key: string, pal: PalData): WikiIcon | null {
	const src = assetLoader.loadPalImage(key, pal.is_pal ?? true);
	return src ? { src } : null;
}

function elementIcon(element: unknown): WikiIcon | null {
	if (typeof element !== 'string') return null;
	const data = elementsData.elements[element];
	return data ? assetIcon(data.icon, data.color) : null;
}

export function byPaldeckIndex(a: number, b: number): number {
	const aRanked = a > 0;
	const bRanked = b > 0;
	if (aRanked !== bRanked) return aRanked ? -1 : 1;
	return aRanked ? a - b : b - a;
}

function palsWithElement(element: string): WikiRelated[] {
	return Object.entries(palsData.pals)
		.filter(([, pal]) => (pal as PalData).element_types?.includes(element as ElementType))
		.sort((a, b) =>
			byPaldeckIndex((a[1] as PalData).pal_deck_index, (b[1] as PalData).pal_deck_index)
		)
		.map(([key, pal]) => ({
			category: 'pals' as const,
			key,
			label: (pal as PalData).localized_name || key,
			icon: palIcon(key, pal as PalData)
		}));
}

function palsWithActiveSkill(skillKey: string): WikiRelated[] {
	const target = skillKey.split('::').pop()?.toLowerCase() ?? '';
	if (!target) return [];
	return Object.entries(palsData.pals)
		.filter(([, pal]) =>
			Object.keys((pal as PalData).skill_set ?? {}).some(
				(name) => name.split('::').pop()?.toLowerCase() === target
			)
		)
		.sort((a, b) =>
			byPaldeckIndex((a[1] as PalData).pal_deck_index, (b[1] as PalData).pal_deck_index)
		)
		.map(([key, pal]) => ({
			category: 'pals' as const,
			key,
			label: (pal as PalData).localized_name || key,
			icon: palIcon(key, pal as PalData)
		}));
}

function referenceChips(
	refs: unknown,
	category: WikiCategory,
	lookup: (key: string) => Record<string, unknown> | undefined,
	iconPath: string[],
	namePath: string[]
): WikiRelated[] {
	if (!Array.isArray(refs)) return [];
	return refs
		.filter((ref): ref is string => typeof ref === 'string' && ref.length > 0)
		.map((ref) => {
			const record = lookup(ref);
			return {
				category,
				key: ref,
				label: nameOr(ref, record, ...namePath),
				sublabel: ref,
				icon: record ? assetIcon(get(record, ...iconPath)) : null,
				missing: !record
			};
		});
}

function unlockedRecipes(record: Record<string, unknown>): WikiRelated[] {
	return referenceChips(
		get(record, 'details', 'unlock_item_recipes'),
		'items',
		(key) => itemsData.items[key] as unknown as Record<string, unknown> | undefined,
		['details', 'icon'],
		['info', 'localized_name']
	);
}

function unlockedBuildings(record: Record<string, unknown>): WikiRelated[] {
	return referenceChips(
		get(record, 'details', 'unlock_build_objects'),
		'buildings',
		(key) => buildingsData.buildings[key] as unknown as Record<string, unknown> | undefined,
		['icon'],
		[]
	);
}

function palsWithWorkSuitability(suitability: string): WikiRelated[] {
	return Object.entries(palsData.pals)
		.map(([key, pal]) => {
			const level = (pal as PalData).work_suitability?.[suitability as WorkSuitability] ?? 0;
			return { key, pal: pal as PalData, level };
		})
		.filter(({ level }) => level > 0)
		.sort((a, b) => b.level - a.level)
		.map(({ key, pal, level }) => ({
			category: 'pals' as const,
			key,
			label: pal.localized_name || key,
			sublabel: `Lv. ${level}`,
			icon: palIcon(key, pal)
		}));
}

export const DESCRIPTORS: Record<WikiCategory, WikiDescriptor> = {
	pals: {
		loadJson: async () => (await import('../../../../data/json/pals.json')).default,
		runtime: () => palsData.pals,
		displayName: (key, record) => nameOr(key, record),
		icon: (key, record) => palIcon(key, record as unknown as PalData),
		cardMeta: (_key, record) => {
			const index = get(record, 'pal_deck_index');
			return typeof index === 'number' && index > 0 ? `#${index}` : null;
		},
		fields: [
			{ label: 'Tribe', value: (r) => field(r, 'tribe') },
			{ label: 'Size', value: (r) => field(r, 'size') },
			{ label: 'Rarity', value: (r) => field(r, 'rarity') },
			{ label: 'Price', value: (r) => field(r, 'price') }
		]
	},
	items: {
		loadJson: async () => (await import('../../../../data/json/items.json')).default,
		runtime: () => itemsData.items,
		displayName: (key, record) => nameOr(key, record, 'info', 'localized_name'),
		description: descriptionField('info', 'description'),
		icon: iconField('details', 'icon'),
		iconBackground: (_key, record) => {
			const rarity = get(record, 'details', 'rarity');
			return typeof rarity === 'number' ? rarityGradientClass(rarity) : null;
		},
		fields: [
			{ label: 'Group', value: (r) => field(r, 'details', 'group') },
			{ label: 'Type', value: (r) => field(r, 'details', 'type_a') },
			{ label: 'Subtype', value: (r) => field(r, 'details', 'type_b') },
			{ label: 'Rank', value: (r) => field(r, 'details', 'rank') },
			{ label: 'Rarity', value: (r) => field(r, 'details', 'rarity') },
			{ label: 'Max Stack', value: (r) => field(r, 'details', 'max_stack_count') },
			{ label: 'Weight', value: (r) => field(r, 'details', 'weight') },
			{ label: 'Price', value: (r) => field(r, 'details', 'price') },
			{ label: 'Sort ID', value: (r) => field(r, 'details', 'sort_id') }
		]
	},
	buildings: {
		loadJson: async () => (await import('../../../../data/json/buildings.json')).default,
		runtime: () => buildingsData.buildings,
		displayName: (key, record) => nameOr(key, record),
		description: descriptionField('description'),
		icon: iconField('icon'),
		fields: [
			{ label: 'Type', value: (r) => field(r, 'type_a') },
			{ label: 'Subtype', value: (r) => field(r, 'type_b') },
			{ label: 'Rank', value: (r) => field(r, 'rank') },
			{ label: 'Required Work', value: (r) => field(r, 'required_build_work_amount') },
			{ label: 'Energy Type', value: (r) => field(r, 'required_energy_type') },
			{ label: 'Materials', value: (r) => field(r, 'materials') }
		]
	},
	'active-skills': {
		loadJson: async () => (await import('../../../../data/json/active_skills.json')).default,
		runtime: () => activeSkillsData.activeSkills,
		displayName: (key, record) => nameOr(key, record),
		description: descriptionField('description'),
		icon: (_key, record) => elementIcon(get(record, 'details', 'element')),
		cardMeta: (_key, record) => {
			const power = get(record, 'details', 'power');
			const cooldown = get(record, 'details', 'cool_time');
			const parts: string[] = [];
			if (typeof power === 'number') parts.push(`${power} pow`);
			if (typeof cooldown === 'number') parts.push(`${cooldown}s`);
			return parts.length > 0 ? parts.join(' · ') : null;
		},
		related: (key, record) => {
			const element = get(record, 'details', 'element');
			const chips: WikiRelated[] = [];
			if (typeof element === 'string' && element.length > 0) {
				chips.push({
					category: 'elements',
					key: element,
					label: elementsData.elements[element]?.localized_name || element,
					icon: elementIcon(element)
				});
			}
			return [...chips, ...palsWithActiveSkill(key)];
		},
		fields: [
			{ label: 'Element', value: (r) => field(r, 'details', 'element') },
			{ label: 'Type', value: (r) => field(r, 'details', 'type') },
			{ label: 'Power', value: (r) => field(r, 'details', 'power') },
			{ label: 'Min Range', value: (r) => field(r, 'details', 'min_range') },
			{ label: 'Max Range', value: (r) => field(r, 'details', 'max_range') },
			{ label: 'Cooldown', value: (r) => field(r, 'details', 'cool_time') },
			{ label: 'Effects', value: (r) => field(r, 'details', 'effects') }
		]
	},
	'passive-skills': {
		loadJson: async () => (await import('../../../../data/json/passive_skills.json')).default,
		runtime: () => passiveSkillsData.passiveSkills,
		displayName: (key, record) => nameOr(key, record),
		description: descriptionField('description'),
		icon: (_key, record) => {
			const rank = get(record, 'details', 'rank');
			if (typeof rank !== 'number') return null;
			const icon = assetIcon(`rank_${rank}`);
			return icon ? { ...icon, filter: skillFilter(rank) } : null;
		},
		fields: [
			{ label: 'Rank', value: (r) => field(r, 'details', 'rank') },
			{ label: 'Effects', value: (r) => field(r, 'details', 'effects') },
			{ label: 'Active Party', value: (r) => field(r, 'details', 'invoke_active_party') },
			{ label: 'Worker', value: (r) => field(r, 'details', 'invoke_worker') },
			{ label: 'Riding', value: (r) => field(r, 'details', 'invoke_riding') },
			{ label: 'Reserve', value: (r) => field(r, 'details', 'invoke_reserve') },
			{ label: 'In Party', value: (r) => field(r, 'details', 'invoke_in_party') },
			{ label: 'Always', value: (r) => field(r, 'details', 'invoke_always') },
			{ label: 'In Base', value: (r) => field(r, 'details', 'invoke_in_base') }
		]
	},
	technologies: {
		loadJson: async () => (await import('../../../../data/json/technologies.json')).default,
		runtime: () => technologiesData.technologies,
		displayName: (key, record) => nameOr(key, record),
		description: descriptionField('description'),
		icon: iconField('details', 'icon'),
		cardMeta: (_key, record) => {
			const cap = get(record, 'details', 'level_cap');
			return typeof cap === 'number' ? `Lv ${cap}` : null;
		},
		related: (_key, record) => [...unlockedBuildings(record), ...unlockedRecipes(record)],
		fields: [
			{ label: 'Tier', value: (r) => field(r, 'details', 'tier') },
			{ label: 'Level Cap', value: (r) => field(r, 'details', 'level_cap') }
		]
	},
	elements: {
		loadJson: async () => (await import('../../../../data/json/elements.json')).default,
		runtime: () => elementsData.elements,
		displayName: (key, record) => nameOr(key, record),
		icon: (_key, record) => assetIcon(get(record, 'icon'), get(record, 'color')),
		extras: (_key, record) => {
			const parts: WikiExtra[] = [
				{ path: 'fruit_icon', label: 'Fruit' },
				{ path: 'egg_icon', label: 'Egg' }
			]
				.map(({ path, label }) => ({ icon: assetIcon(get(record, path)), label }))
				.filter((entry): entry is { icon: WikiIcon; label: string } => entry.icon !== null)
				.map(({ icon, label }) => ({ src: icon.src, label }));
			return parts;
		},
		related: (key) => palsWithElement(key),
		fields: []
	},
	'work-suitability': {
		loadJson: async () =>
			Object.fromEntries(WORK_SUITABILITY_KEYS.map((key) => [key, {}])),
		runtime: () => workSuitabilityData.workSuitability,
		displayName: (key, record) => nameOr(key, record),
		icon: (key) => assetIcon(suitabilityImageMap[key as WorkSuitability]),
		related: (key) => palsWithWorkSuitability(key),
		fields: [{ label: 'Description', value: (r) => field(r, 'description') }]
	}
};

export function descriptorFor(category: WikiCategory): WikiDescriptor {
	return DESCRIPTORS[category];
}
