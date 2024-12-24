from pydantic import BaseModel


class Settings(BaseModel):
    language: str
