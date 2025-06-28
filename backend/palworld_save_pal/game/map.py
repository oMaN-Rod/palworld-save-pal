from pydantic import BaseModel


class WorldMapPoint(BaseModel):
    x: float
    y: float
    z: float
