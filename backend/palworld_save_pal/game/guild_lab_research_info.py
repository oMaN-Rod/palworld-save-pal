from pydantic import BaseModel


class GuildLabResearchInfo(BaseModel):
    research_id: str
    work_amount: float
