//! WorldOption read/patch handlers. Both require a loaded save; a save with no
//! WorldOption.sav answers `present: false` rather than erroring, so the frontend can
//! disable the button without special-casing an error frame.

use psp_core::domain::world_option::WorldOptionPatch;
use psp_core::dto::world_option::WorldOptionPatchDto;

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

pub async fn handle_get_world_option(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;
    let dto = session.world_option_dto();
    ctx.emitter.emit(MessageType::GetWorldOption, &dto);
    Ok(())
}

pub async fn handle_update_world_option(
    data: WorldOptionPatchDto,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let session = ctx.session.save_mut()?;

    let patch: Vec<WorldOptionPatch> = data
        .entries
        .into_iter()
        .map(|entry| WorldOptionPatch {
            key: entry.key,
            value: entry.value,
        })
        .collect();

    session.apply_world_option_patch(&patch)?;

    // Answers under UpdateWorldOption, not GetWorldOption — the frontend's
    // sendAndWait correlates the response by the request's own message type.
    let dto = session.world_option_dto();
    ctx.emitter.emit(MessageType::UpdateWorldOption, &dto);
    Ok(())
}
