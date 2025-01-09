from pydantic import BaseModel


class RegisteredResponse(BaseModel):
    player_id: int
    player_name: str
    time_to_answer_ms: int

    @classmethod
    def of_player(cls, player, duration: int):
        return cls(
            player_id=player.id, player_name=player.name, time_to_answer_ms=duration
        )
