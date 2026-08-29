import { SITE_ORIGIN } from '$lib/i18n/routingConfig.js';

export type Crumb = { name: string; path: string };
export type FaqEntry = { question: string; answer: string };
export type ListItem = { name: string; path: string };

const absolute = (path: string): string => `${SITE_ORIGIN}${path}`;

export function webApplicationSchema() {
	return {
		'@type': 'WebApplication',
		name: 'Palworld Save Pal',
		applicationCategory: 'GameApplication',
		operatingSystem: 'Web, Windows, macOS, Linux',
		browserRequirements: 'Requires WebAssembly and Web Workers.',
		isAccessibleForFree: true,
		offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' }
	};
}

export function faqPageSchema(entries: FaqEntry[]) {
	return {
		'@type': 'FAQPage',
		mainEntity: entries.map((entry) => ({
			'@type': 'Question',
			name: entry.question,
			acceptedAnswer: { '@type': 'Answer', text: entry.answer }
		}))
	};
}

export function breadcrumbSchema(trail: Crumb[]) {
	return {
		'@type': 'BreadcrumbList',
		itemListElement: trail.map((crumb, index) => ({
			'@type': 'ListItem',
			position: index + 1,
			name: crumb.name,
			item: absolute(crumb.path)
		}))
	};
}

export function itemListSchema(name: string, items: ListItem[]) {
	return {
		'@type': 'ItemList',
		name,
		numberOfItems: items.length,
		itemListElement: items.map((item, index) => ({
			'@type': 'ListItem',
			position: index + 1,
			name: item.name,
			url: absolute(item.path)
		}))
	};
}
