import * as m from '$i18n/messages';
import { MAX_UPLOAD_BYTES } from '$lib/utils/modUpload';
import type { ModError } from '$types';
import { formatSize } from './modList';

export function uploadErrorText(error: ModError): string {
	switch (error.code) {
		case 'invalid_name':
			return m.mods_upload_invalid_name();
		case 'unsupported_type':
			return m.mods_upload_unsupported_type();
		case 'invalid_size':
			return m.mods_upload_empty();
		case 'upload_too_large':
			return m.mods_upload_too_large({
				max: formatSize(typeof error.max_bytes === 'number' ? error.max_bytes : MAX_UPLOAD_BYTES)
			});
		case 'too_many_uploads':
			return m.mods_upload_too_many();
		case 'invalid_hash':
		case 'chunk_out_of_order':
		case 'invalid_chunk':
		case 'chunk_too_large':
			return m.mods_upload_protocol();
		case 'upload_not_found':
			return m.mods_upload_expired();
		case 'size_exceeded':
		case 'size_mismatch':
		case 'hash_mismatch':
			return m.mods_upload_corrupt();
		case 'io':
			return m.mods_upload_io({ message: error.message });
		case 'read_failed':
			return m.mods_upload_read_failed({ message: error.message });
		case 'connection_lost':
			return m.mods_upload_connection_lost();
		default:
			return error.message;
	}
}
