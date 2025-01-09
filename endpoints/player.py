from fastapi import APIRouter
from starlette import status
from starlette.responses import JSONResponse

from core.exceptions import DuplicateValue, UnknownPlayerError
from core.simple_game_manager import game_manager
from endpoints.schema.player_responses import (
    EveryoneAnswerOrderResponse,
    PlayerAnswerResponse,
)
from models.player import Player

player_router = APIRouter(tags=["player_side"])


@player_router.get("/players", response_model=list[Player], summary="Get all players")
async def get_players():
    return game_manager.get_players()


@player_router.post(
    "/join",
    response_model=Player,
    status_code=status.HTTP_201_CREATED,
    summary="Join the game as player",
)
async def add_player(name: str):
    player = Player.create_by_name(name=name)
    await game_manager.add_player(player)
    return player


@player_router.post("/request_answer", status_code=status.HTTP_201_CREATED)
async def request_answer(
    player_id: int,
):
    try:
        player = await game_manager.get_player_by_id(player_id)
    except UnknownPlayerError:
        return JSONResponse(
            status_code=status.HTTP_404_NOT_FOUND, content={"error": "Unknown player"}
        )
    try:
        await game_manager.request_answer(player)
    except DuplicateValue:
        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content={"error": "You have already answered"},
        )

    return PlayerAnswerResponse(answered=True)


@player_router.get("/check_answer_order")
async def check_answer_order() -> EveryoneAnswerOrderResponse:
    order = await game_manager.get_answers_order()

    return EveryoneAnswerOrderResponse(answers=order)
