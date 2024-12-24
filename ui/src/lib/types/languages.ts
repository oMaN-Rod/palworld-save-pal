export type SupportedLanguage =
	| 'de'
	| 'en'
	| 'es'
	| 'fr'
	| 'it'
	| 'ko'
	| 'pt-BR'
	| 'ru'
	| 'zh-Hans'
	| 'zh-Hant';

export const languages: Record<SupportedLanguage, string> = {
	de: 'Deutsch',
	en: 'English',
	es: 'Español',
	fr: 'Français',
	it: 'Italiano',
	ko: '한국어',
	'pt-BR': 'Português',
	ru: 'Русский',
	'zh-Hans': '简体中文',
	'zh-Hant': '繁體中文'
};
