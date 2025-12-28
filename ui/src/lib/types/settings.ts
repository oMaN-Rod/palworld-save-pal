export type SupportedLanguage =
	| 'de'
	| 'en'
	| 'es'
	| 'es-MX'
	| 'fr'
	| 'id'
	| 'it'
	| 'ko'
	| 'pl'
	| 'pt-BR'
	| 'ru'
	| 'th'
	| 'tr'
	| 'vi'
	| 'zh-Hans'
	| 'zh-Hant';

export const languages: Record<SupportedLanguage, string> = {
	de: 'Deutsch',
	en: 'English',
	es: 'Español',
	'es-MX': 'Español (México)',
	fr: 'Français',
	id: 'Bahasa Indonesia',
	it: 'Italiano',
	ko: '한국어',
	pl: 'Polski',
	'pt-BR': 'Português',
	ru: 'Русский',
	th: 'ไทย',
	tr: 'Türkçe',
	vi: 'Tiếng Việt',
	'zh-Hans': '简体中文',
	'zh-Hant': '繁體中文'
};

export interface AppSettings {
	language: SupportedLanguage;
	save_dir?: string;
	clone_prefix?: string;
	new_pal_prefix?: string;
	debug_mode?: boolean;
	cheat_mode?: boolean;
}
