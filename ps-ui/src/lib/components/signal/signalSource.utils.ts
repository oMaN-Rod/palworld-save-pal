import type { SelectSignalSourceKind, SetSignalSourceRequest, SignalSourceKind } from '$types';

export function sourceKindFromStatus(kind: SignalSourceKind | undefined): SelectSignalSourceKind {
	if (kind === 'file') return 'file';
	if (kind === 'rest') return 'server';
	return 'off';
}

export interface BuildSetSourceRequestOptions {
	filePath: string;
	selectedServerId: number | '';
}

export function buildSetSourceRequest(
	kind: SelectSignalSourceKind,
	{ filePath, selectedServerId }: BuildSetSourceRequestOptions
): SetSignalSourceRequest | null {
	if (kind === 'off') return { kind: 'off' };
	if (kind === 'file') return { kind: 'file', path: filePath.trim() || undefined };
	if (selectedServerId !== '') return { kind: 'server', server_id: Number(selectedServerId) };
	return null;
}
