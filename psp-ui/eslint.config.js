import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

/** @type {import('eslint').Linter.FlatConfig[]} */
export default [
	js.configs.recommended,
	...ts.configs.recommended,
	...svelte.configs['flat/recommended'],
	prettier,
	...svelte.configs['flat/prettier'],
	{
		languageOptions: {
			globals: {
				...globals.browser,
				...globals.node
			}
		}
	},
	{
		files: ['**/*.svelte'],
		languageOptions: {
			parserOptions: {
				parser: ts.parser
			}
		}
	},
	{
		ignores: [
			'build/',
			'.svelte-kit/',
			'dist/',
			// Generated: paraglide compiler output (rewritten on every sync/build)
			'src/paraglide/',
			// Generated: wasm-pack output
			'src/lib/wasm/',
			// Vendored minified Draco decoder (three.js mesh decompression)
			'static/draco/',
			// Vendored emscripten build of the Ooz decompressor
			'vendor/ooz/**/*.mjs'
		]
	}
];
