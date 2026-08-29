import { describe, expect, it } from 'vitest';
import { loadCategorySeo, loadEntitySeo } from './wikiL10n';

describe('wikiL10n', () => {
	it('joins a pal slug to its localized name and description', async () => {
		const entity = await loadEntitySeo('pals', 'amaterasu-wolf');
		expect(entity?.key).toBe('AmaterasuWolf');
		expect(entity?.name).toBe('Kitsun');
		expect(entity?.description).toContain('Kitsun');
	});

	it('resolves prefixed keys for active skills', async () => {
		const entity = await loadEntitySeo('active-skills', 'acid-rain', { stripPrefix: true });
		expect(entity?.key).toBe('EPalWazaID::AcidRain');
		expect(entity?.name).toBe('Acid Rain');
	});

	it('returns null for an unknown slug', async () => {
		expect(await loadEntitySeo('pals', 'not-a-real-pal')).toBeNull();
	});

	it('tolerates categories whose l10n descriptions are null', async () => {
		const entity = await loadEntitySeo('elements', 'dark', { stripPrefix: true });
		expect(entity?.name).toBe('Dark');
		expect(entity?.description).toBeNull();
	});

	it('lists every enabled entity in a category', async () => {
		const all = await loadCategorySeo('pals');
		expect(all.length).toBeGreaterThan(700);
		expect(all.every((entity) => entity.name.length > 0)).toBe(true);
	});
});
