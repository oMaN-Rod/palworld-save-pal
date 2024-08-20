from uuid import UUID


def custom_uuid_encoder(uuid: UUID) -> str:
    return str(uuid)
