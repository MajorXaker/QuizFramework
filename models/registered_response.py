from pydantic import BaseModel


class RegisteredResponse(BaseModel):
    player_id: int
    time_to_answer_ms: int