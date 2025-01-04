from fastapi import APIRouter, status, Depends

from core.simple_game_manager import game, SimpleGameManager
from endpoints.auth import auth

admin_router = APIRouter()

@admin_router.post("/run", dependencies=[Depends(auth)])
async def restart_game(
    max_rounds: int = None,
):
    game.restart_game(rounds=max_rounds)

    return {"restarted": "ok"}, status.HTTP_200_OK

@admin_router.post("/close", dependencies=[Depends(auth)])
async def close_registration():
    game.prohibit_player_join()

    return {"registration": "closed"}, status.HTTP_200_OK

@admin_router.post("/open", dependencies=[Depends(auth)])
async def open_registration():
    game.allow_player_join()

    return {"registration": "open"}, status.HTTP_200_OK

@admin_router.post("/next_question", dependencies=[Depends(auth)])
async def next_question():
    game.next_round()

    return {"next_question": "ok"}, status.HTTP_200_OK
