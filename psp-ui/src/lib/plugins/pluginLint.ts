const DECLARED = /^\s*function\s+([A-Za-z_]\w*)\s*\(/;
const ASSIGNED = /^\s*([A-Za-z_]\w*)\s*=\s*function\s*\(/;

/** Only whole-line `--` comments are skipped, so this errs toward an extra warning rather than a missed one. */
export function globalFunctionNames(source: string): string[] {
	const names: string[] = [];
	for (const line of source.split('\n')) {
		if (line.trimStart().startsWith('--')) continue;
		const match = DECLARED.exec(line) ?? ASSIGNED.exec(line);
		if (match && !names.includes(match[1])) names.push(match[1]);
	}
	return names;
}

export interface AgreementWarning {
	kind: 'command-without-function' | 'function-without-command';
	name: string;
	message: string;
}

export function commandAgreement(
	commandIds: readonly string[],
	source: string
): AgreementWarning[] {
	const defined = globalFunctionNames(source);
	const warnings: AgreementWarning[] = [];

	for (const id of commandIds) {
		if (!defined.includes(id)) {
			warnings.push({
				kind: 'command-without-function',
				name: id,
				message: `The manifest declares the command "${id}", but this script defines no global function of that name. Running it will fail.`
			});
		}
	}
	for (const name of defined) {
		if (!commandIds.includes(name)) {
			warnings.push({
				kind: 'function-without-command',
				name,
				message: `"${name}" is a global function no manifest command names, so nothing can run it.`
			});
		}
	}
	return warnings;
}
