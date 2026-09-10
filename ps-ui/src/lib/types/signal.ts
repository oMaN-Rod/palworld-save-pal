
export type SignalSourceKind = 'file' | 'rest';
export type SignalSourceHealth = 'idle' | 'waiting' | 'auth' | 'down' | 'stale' | 'ok';

export interface SignalSourceStatus {
	kind?: SignalSourceKind;
	health: SignalSourceHealth;
	error?: string;
	lastFrameMs?: number;
	actorCount: number;
}

export type PairingLabel = 'off' | 'waiting' | 'connected' | 'failed';

export interface ConnectedDeviceJson {
	deviceId: string;
	name: string;
}

export interface SignalStatusJson {
	source: SignalSourceStatus;
	pairing: PairingLabel;
	code?: string;
	expiresAtMs?: number;
	armed: boolean;
	connectedDevice?: ConnectedDeviceJson;
}

export interface SignalStartPairingJson extends SignalStatusJson {
	url?: string;
}

export interface SignalErrorReply {
	error: string;
}

export type SelectSignalSourceKind = 'off' | 'file' | 'server';

export interface SetSignalSourceRequest {
	kind: SelectSignalSourceKind;
	path?: string;
	server_id?: number;
}

export interface TurnServerConfig {
	urls: string[];
	username?: string;
	credential?: string;
}

export interface StartPairingRequest {
	turn?: TurnServerConfig;
}

export interface SignalDeviceJson {
	deviceId: string;
	name: string;
	createdAtMs: number;
	lastSeenMs?: number;
	connected: boolean;
}

export interface SignalDeviceListJson {
	devices: SignalDeviceJson[];
}

export interface SetArmedRequest {
	armed: boolean;
}

export interface RenameDeviceRequest {
	device_id: string;
	name: string;
}

export interface DeviceIdRequest {
	device_id: string;
}
