//! Wire handlers that receive an archive as a sequence of base64 chunks.
use base64::Engine;
use serde_json::{json, Value};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;
use crate::mods_handlers::emit_refusal;
use crate::services::mods::uploads::{
    UploadError, UploadStore, MAX_PENDING_UPLOADS, MAX_UPLOAD_BYTES,
};

#[derive(Debug, serde::Deserialize)]
pub struct UploadBeginData {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UploadChunkData {
    pub upload_id: String,
    pub seq: u64,
    pub data_b64: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UploadEndData {
    pub upload_id: String,
}

pub async fn handle_mod_upload_begin(
    uploads: &UploadStore,
    data: UploadBeginData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match uploads.begin(&data.name, data.size, &data.sha256, ctx.emitter) {
        Ok((upload_id, chunk_size)) => ctx.emitter.emit(
            MessageType::ModUploadBegin,
            &json!({ "upload_id": upload_id, "chunk_size": chunk_size, "name": data.name }),
        ),
        Err(error) => refuse(
            ctx,
            MessageType::ModUploadBegin,
            json!({ "name": data.name }),
            &error,
        ),
    }
    Ok(())
}

pub async fn handle_mod_upload_chunk(
    uploads: &UploadStore,
    data: UploadChunkData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let context = json!({ "upload_id": data.upload_id, "seq": data.seq });
    let bytes = match base64::engine::general_purpose::STANDARD.decode(&data.data_b64) {
        Ok(bytes) => bytes,
        Err(error) => {
            emit_refusal(
                ctx,
                MessageType::ModUploadChunk,
                context,
                "invalid_chunk",
                format!("data_b64 is not base64: {error}"),
                json!({}),
            );
            return Ok(());
        }
    };
    match uploads.chunk(&data.upload_id, data.seq, &bytes) {
        Ok(received) => ctx.emitter.emit(
            MessageType::ModUploadChunk,
            &json!({ "upload_id": data.upload_id, "seq": data.seq, "received": received }),
        ),
        Err(error) => refuse(ctx, MessageType::ModUploadChunk, context, &error),
    }
    Ok(())
}

pub async fn handle_mod_upload_end(
    uploads: &UploadStore,
    data: UploadEndData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    match uploads.end(&data.upload_id) {
        Ok(path) => {
            let path = ps_core::mods::native_separators(&path.to_string_lossy(), cfg!(windows));
            ctx.emitter.emit(
                MessageType::ModUploadEnd,
                &json!({ "upload_id": data.upload_id, "path": path }),
            );
        }
        Err(error) => refuse(
            ctx,
            MessageType::ModUploadEnd,
            json!({ "upload_id": data.upload_id }),
            &error,
        ),
    }
    Ok(())
}

fn refuse(
    ctx: &mut HandlerCtx<'_>,
    message_type: MessageType,
    context: Value,
    error: &UploadError,
) {
    let detail = match error {
        UploadError::OutOfOrder { expected } => json!({ "expected_seq": expected }),
        UploadError::SizeMismatch { expected, received } => {
            json!({ "expected": expected, "received": received })
        }
        UploadError::TooLarge => json!({ "max_bytes": MAX_UPLOAD_BYTES }),
        UploadError::TooManyUploads => json!({ "max_pending": MAX_PENDING_UPLOADS }),
        _ => json!({}),
    };
    emit_refusal(
        ctx,
        message_type,
        context,
        error.code(),
        error.to_string(),
        detail,
    );
}
