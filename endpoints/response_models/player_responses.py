from pydantic import BaseModel
from fastapi import status

class AnswerResponse(BaseModel):
    status: status
    time_to_answer_ms: int
    your_order: int