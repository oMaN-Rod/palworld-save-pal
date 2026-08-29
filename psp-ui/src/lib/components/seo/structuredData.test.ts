import { describe, expect, it } from 'vitest';
import {
	breadcrumbSchema,
	faqPageSchema,
	itemListSchema,
	webApplicationSchema
} from './structuredData';

describe('structuredData', () => {
	it('describes the editor as a free web application', () => {
		const schema = webApplicationSchema();
		expect(schema['@type']).toBe('WebApplication');
		expect(schema.applicationCategory).toBe('GameApplication');
		expect(schema.offers).toMatchObject({ price: '0', priceCurrency: 'USD' });
	});

	it('builds an FAQPage from question/answer pairs', () => {
		const schema = faqPageSchema([{ question: 'Is it free?', answer: 'Yes.' }]);
		expect(schema['@type']).toBe('FAQPage');
		expect(schema.mainEntity).toHaveLength(1);
		expect(schema.mainEntity[0]).toMatchObject({
			'@type': 'Question',
			name: 'Is it free?',
			acceptedAnswer: { '@type': 'Answer', text: 'Yes.' }
		});
	});

	it('numbers breadcrumb positions from one and absolutizes URLs', () => {
		const schema = breadcrumbSchema([
			{ name: 'Wiki', path: '/wiki' },
			{ name: 'Pals', path: '/wiki/pals' }
		]);
		expect(schema.itemListElement[0]).toMatchObject({ position: 1, name: 'Wiki' });
		expect(schema.itemListElement[1].item).toBe('https://palworldsavepal.app/wiki/pals');
	});

	it('builds an ItemList with absolute urls', () => {
		const schema = itemListSchema('Pals', [{ name: 'Lamball', path: '/wiki/pals/lamball' }]);
		expect(schema['@type']).toBe('ItemList');
		expect(schema.numberOfItems).toBe(1);
		expect(schema.itemListElement[0].url).toBe('https://palworldsavepal.app/wiki/pals/lamball');
	});
});
