from typing import AsyncIterator

from fastapi import APIRouter, Request, UploadFile, File
from fastapi.responses import StreamingResponse, Response

from palworld_save_pal.game.save_file import SaveFile
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)

router = APIRouter(prefix="/api/convert")


async def _iter_json_chunks(
    json_str: str, chunk_size: int = 64 * 1024
) -> AsyncIterator[bytes]:
    for i in range(0, len(json_str), chunk_size):
        yield json_str[i : i + chunk_size].encode("utf-8")


@router.post("/sav-to-json")
async def sav_to_json(file: UploadFile = File(...)):
    logger.info("API: Converting SAV to JSON")
    file_data = await file.read()
    json_str = SaveFile().convert_sav_file_to_json(file_data)
    return StreamingResponse(
        _iter_json_chunks(json_str),
        media_type="application/json",
        headers={"Content-Length": str(len(json_str.encode("utf-8")))},
    )


@router.post("/json-to-sav")
async def json_to_sav(request: Request):
    logger.info("API: Converting JSON to SAV")
    body = await request.body()
    sav_data = SaveFile().convert_json_to_sav_file(body)
    return Response(
        content=sav_data,
        media_type="application/octet-stream",
        headers={
            "Content-Disposition": "attachment; filename=converted.sav",
            "Content-Length": str(len(sav_data)),
        },
    )
