from fastapi import status
from pydantic import BaseModel

from models.registered_response import RegisteredResponse


class EveryoneAnswerOrderResponse(BaseModel):
    answers: list[RegisteredResponse]

    def __dict__(self):
        return self.model_dump(exclude_none=True)


class PlayerAnswerResponse(BaseModel):
    answered: bool
    error: str = None
