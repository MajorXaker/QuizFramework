from pydantic import BaseModel
import uuid


class Player(BaseModel):
    id: int
    name: str

    @classmethod
    def create_by_name(cls, name: str):
        return Player(id=uuid.uuid4().int, name=name)

    def __hash__(self):
        return self.id
