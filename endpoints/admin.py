from fastapi import APIRouter, Depends

from core.simple_game_manager import game_manager
from endpoints.auth import auth

admin_router = APIRouter(tags=["admin_side"])


@admin_router.post(
    "/admin/restart",
    dependencies=[Depends(auth)],
)
async def restart_game(
    max_rounds: int = None,
):
    await game_manager.restart_game(rounds=max_rounds)

    return {"restarted": "ok"}


@admin_router.post("/admin/close_joining", dependencies=[Depends(auth)])
async def close_registration():
    await game_manager.prohibit_player_join()

    return {"registration": "closed"}


@admin_router.post("/admin/open_joining", dependencies=[Depends(auth)])
async def open_registration():
    await game_manager.allow_player_join()

    return {"registration": "open"}


@admin_router.post("/admin/next_question", dependencies=[Depends(auth)])
async def next_question():
    await game_manager.next_round()

    return {"next_question": "ok"}


@admin_router.post("/admin/end_game", dependencies=[Depends(auth)])
async def end_game():
    await game_manager.end_game()

    return {"game": "ended"}
