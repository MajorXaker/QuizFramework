from datetime import datetime, timezone

import pytest
from models.player import Player
from core.exceptions import UnknownPlayerError, JoiningRestrictedError, GameEnded


@pytest.mark.asyncio
async def test_restart_game(live_game_manager):
    await live_game_manager.add_player(Player(id=1, name="test"))
    await live_game_manager.restart_game()
    assert len(await live_game_manager.get_players()) == 0


@pytest.mark.asyncio
async def test_add_player(live_game_manager):
    player = Player(id=1, name="test")
    await live_game_manager.add_player(player)
    assert await live_game_manager.get_players() == [player]


@pytest.mark.asyncio
async def test_end_game(live_game_manager):
    await live_game_manager.add_player(Player(id=1, name="test"))
    await live_game_manager.end_game()
    assert len(await live_game_manager.get_players()) == 0


@pytest.mark.asyncio
async def test_request_answer(live_game_manager):
    start_time = datetime.now(timezone.utc)
    await live_game_manager.restart_game()
    await live_game_manager.next_round()
    player = Player(id=1, name="test")
    await live_game_manager.add_player(player)
    response = await live_game_manager.request_answer(player)
    duration = (datetime.now(timezone.utc) - start_time).microseconds
    assert response.player_id == player.id
    # The time to answer should be less than the duration
    assert response.time_to_answer_ms < duration


@pytest.mark.asyncio
async def test_request_answer_unknown_player(live_game_manager):
    player = Player(id=1, name="test")
    with pytest.raises(UnknownPlayerError):
        await live_game_manager.request_answer(player)


@pytest.mark.asyncio
async def test_next_round(live_game_manager):
    await live_game_manager.restart_game(rounds=2)
    await live_game_manager.next_round()
    assert live_game_manager._current_round == 1


@pytest.mark.asyncio
async def test_join_player_registration_closed(live_game_manager):
    await live_game_manager.prohibit_player_join()
    with pytest.raises(JoiningRestrictedError):
        await live_game_manager.add_player(Player(id=1, name="test"))


@pytest.mark.asyncio
async def test_join_game_not_started(live_game_manager):
    await live_game_manager.end_game()
    with pytest.raises(GameEnded):
        await live_game_manager.add_player(Player(id=1, name="test"))
