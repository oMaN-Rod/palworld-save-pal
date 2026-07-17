export type ServerType = 'docker' | 'native';
export type ContainerStatus = 'running' | 'exited' | 'created' | 'paused' | 'not_found';

export interface ServerStatus {
	status: ContainerStatus;
	running: boolean;
	started_at?: string;
	health?: string;
}

export interface Server {
	id: number;
	name: string;
	container_name: string;
	image_name: string;
	server_type: ServerType;
	game_port: number;
	query_port: number;
	rest_api_port: number;
	data_volume_name: string;
	saves_path: string;
	mods_path: string;
	logicmods_path: string;
	nativemods_path: string;
	install_path: string;
	steamcmd_path: string;
	pid: number | null;
	launch_args: string;
	workshop_dir: string;
	server_name: string;
	server_description: string;
	server_password: string;
	admin_password: string;
	max_players: number;
	env_vars: Record<string, any>;
	created_at: string;
	updated_at: string;
	status?: ServerStatus;
	player_count?: number;
	total_players?: number;
}

export interface ServerMod {
	mod_name: string;
	display_name?: string;
	mod_type: 'ue4ss' | 'logic' | 'native' | 'lua' | 'palschema' | 'paks' | 'unknown';
	mod_version?: string;
	mod_author?: string;
	source?: 'workshop' | 'local' | 'config';
	enabled: boolean;
}

export interface CreateServerData {
	name: string;
	container_name: string;
	image_name?: string;
	server_type: ServerType;
	game_port: number;
	query_port: number;
	rest_api_port: number;
	server_name?: string;
	server_description?: string;
	server_password?: string;
	admin_password?: string;
	max_players?: number;
	env_vars?: Record<string, any>;
	steamcmd_path?: string;
	install_path?: string;
	launch_args?: string;
	workshop_dir?: string;
}

export interface ImportServerData {
	install_path: string;
	name: string;
	query_port?: number;
	launch_args?: string;
	workshop_dir?: string;
}

export interface ContainerStats {
	cpu_percent: number;
	mem_usage_mb: number;
	mem_limit_mb: number;
	mem_percent: number;
	net_rx_mb: number;
	net_tx_mb: number;
	disk_read_mb: number;
	disk_write_mb: number;
}

export interface ServerApiResponse {
	server_id: number;
	endpoint: string;
	result: {
		status_code: number;
		data: any;
	};
}
