from fastapi import APIRouter, status, Depends

from core.simple_game_manager import SimpleGameManager
from fastapi import Body, Query
from models.player import Player
player_router = APIRouter()

@player_router.get("/players", response_model=list[Player])
async def get_players(manager: SimpleGameManager = Depends()):
    return manager.get_players()


@player_router.post("/players", response_model=Player)
async def add_player(name: str = Body(..., embed=True), manager: SimpleGameManager = Depends()):
    player = Player.create_by_name(name=name)
    manager.add_player(player)
    return player

@player_router.post("/request_answer", response_model=...)
async def request_answer(player_id: int = Query(...), manager: SimpleGameManager = Depends()):
    manager.request_answer(Player(id=player_id))
    return status.HTTP_200_OK