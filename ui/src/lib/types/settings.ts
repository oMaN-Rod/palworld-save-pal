export type SupportedLanguage =
	| 'de'
	| 'en'
	| 'es'
	| 'es-mx'
	| 'fr'
	| 'it'
	| 'id-id'
	| 'ko'
	| 'pl'
	| 'pt-br'
	| 'ru'
	| 'th'
	| 'tr'
	| 'vi'
	| 'zh-hans'
	| 'zh-hant';

export const languages: Record<SupportedLanguage, string> = {
	de: 'Deutsch',
	en: 'English',
	es: 'Español',
	'es-mx': 'Español (México)',
	fr: 'Français',
	'id-id': 'Bahasa Indonesia',
	it: 'Italiano',
	ko: '한국어',
	pl: 'Polski',
	'pt-br': 'Português',
	ru: 'Русский',
	th: 'ไทย',
	tr: 'Türkçe',
	vi: 'Tiếng Việt',
	'zh-hans': '简体中文',
	'zh-hant': '繁體中文'
};

export interface AppSettings {
	language: SupportedLanguage;
	save_dir?: string;
	clone_prefix?: string;
	new_pal_prefix?: string;
	debug_mode?: boolean;
	cheat_mode?: boolean;
}
