export type PluginCapability =
	| 'save.read'
	| 'save.write'
	| 'save.raw'
	| 'players'
	| 'gamedata'
	| 'ui.dialog'
	| 'storage'
	| 'log';

export type ApiType =
	| { kind: 'nil' }
	| { kind: 'boolean' }
	| { kind: 'integer' }
	| { kind: 'number' }
	| { kind: 'string' }
	| { kind: 'table' }
	| { kind: 'any' }
	| { kind: 'handle'; value: string }
	| { kind: 'iterator'; value: string }
	| { kind: 'union'; value: ApiType[] }
	| { kind: 'list'; value: ApiType }
	| { kind: 'map'; value: { key: ApiType; value: ApiType } };

export type ApiAccess = 'read_write' | 'read_only';

export interface ApiParam {
	name: string;
	type: ApiType;
	optional: boolean;
}

export interface ApiFunction {
	name: string;
	params: ApiParam[];
	returns: ApiType;
	doc: string;
	capability: PluginCapability | null;
}

export interface ApiField {
	name: string;
	type: ApiType;
	access: ApiAccess;
	doc: string;
}

export interface ApiGlobal {
	name: string;
	capability: PluginCapability | null;
	functions: ApiFunction[];
	fields: ApiField[];
}

export interface ApiHandle {
	name: string;
	fields: ApiField[];
	methods: ApiFunction[];
	capability: PluginCapability | null;
}

export interface ApiDefinition {
	globals: ApiGlobal[];
	handles: ApiHandle[];
}

export function typeName(type: ApiType): string {
	switch (type.kind) {
		case 'handle':
			return type.value;
		case 'iterator':
			return `fun(): ${type.value}|nil`;
		case 'union':
			return type.value.map(typeName).join('|');
		case 'list':
			return `${typeName(type.value)}[]`;
		case 'map':
			return `table<${typeName(type.value.key)}, ${typeName(type.value.value)}>`;
		default:
			return type.kind;
	}
}

export function effectiveCapability(
	own: PluginCapability | null,
	owner: PluginCapability | null
): PluginCapability | null {
	return own ?? owner;
}

export function isGranted(
	capability: PluginCapability | null,
	granted: readonly PluginCapability[]
): boolean {
	return capability === null || granted.includes(capability);
}

export function visibleGlobals(
	definition: ApiDefinition,
	granted: readonly PluginCapability[]
): ApiGlobal[] {
	return definition.globals
		.filter((global) => isGranted(global.capability, granted))
		.map((global) => ({
			...global,
			functions: global.functions.filter((fn) =>
				isGranted(effectiveCapability(fn.capability, global.capability), granted)
			),
			fields: [...global.fields]
		}));
}

export function globalByName(definition: ApiDefinition, name: string): ApiGlobal | undefined {
	return definition.globals.find((global) => global.name === name);
}

export function handleByName(definition: ApiDefinition, name: string): ApiHandle | undefined {
	return definition.handles.find((handle) => handle.name === name);
}

export function signatureLabel(fn: ApiFunction): string {
	const params = fn.params
		.map((param) => `${param.name}${param.optional ? '?' : ''}: ${typeName(param.type)}`)
		.join(', ');
	return `${fn.name}(${params}): ${typeName(fn.returns)}`;
}

export function functionDoc(fn: ApiFunction, ownerCapability: PluginCapability | null): string {
	const capability = effectiveCapability(fn.capability, ownerCapability);
	return capability === null ? fn.doc : `${fn.doc}\n\nRequires capability: \`${capability}\`.`;
}

export function fieldDoc(field: ApiField): string {
	return `\`${typeName(field.type)}\`\n\n${field.doc}`;
}
