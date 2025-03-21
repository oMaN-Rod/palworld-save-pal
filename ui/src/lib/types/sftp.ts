// Message types enum
export enum MessageType {
    SETUP_SFTP_CONNECTION = 'SETUP_SFTP_CONNECTION',
    NAVIGATE_DIRECTORY = 'NAVIGATE_DIRECTORY'
}

// File/Directory item interface
export interface SFTPFileItem {
    name: string;
    is_dir: boolean;
}

// Request interfaces
export interface SFTPConnectionRequest {
    hostname: string;
    username: string;
    password: string;
}

export interface NavigateDirectoryRequest {
    path: string;
}

// Response interfaces
export interface BaseResponse {
    success: boolean;
    message: string;
}

export interface SFTPConnectionResponse extends BaseResponse {
    files: SFTPFileItem[];
    path: string;
}

export interface NavigateDirectoryResponse extends BaseResponse {
    files: SFTPFileItem[];
    path: string;
}

// Combined type for all possible responses
export type WebSocketResponse = 
    | SFTPConnectionResponse 
    | NavigateDirectoryResponse;

// Type guard to check if response is SFTP related
export function isSFTPResponse(response: any): response is SFTPConnectionResponse | NavigateDirectoryResponse {
    return (
        'success' in response &&
        'message' in response &&
        'files' in response &&
        'path' in response &&
        Array.isArray(response.files) &&
        response.files.every((file: any) => 
            'name' in file && 
            'is_dir' in file &&
            typeof file.name === 'string' &&
            typeof file.is_dir === 'boolean'
        )
    );
}
