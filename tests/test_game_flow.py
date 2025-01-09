import base64
from datetime import datetime, timezone

import pytest

from endpoints.player import request_answer
from models.player import Player
from core.exceptions import UnknownPlayerError

from config import settings as s

restart_game_url = "http://test/admin/restart"
end_game_url = "http://test/admin/end_game"
next_question_url = "http://test/admin/next_question"
join_game_url = "http://test/join"
request_answer_url = "http://test/request_answer"
check_answer_order_url = "http://test/check_answer_order"


@pytest.mark.asyncio
async def test_basic_game_flow(live_game_manager, client, patch_auth):
    auth_header = f"Basic {base64.b64encode(f'{s.REST_LOGIN}:{s.REST_PASSWORD}'.encode()).decode()}"
    resp = await client.post(
        restart_game_url,
        headers={"Authorization": auth_header},
    )
    assert resp.status_code == 200
    assert resp.json() == {"restarted": "ok"}

    player_1_name = "Vasya"
    player_2_name = "Petya"

    p1_resp = await client.post(
        join_game_url,
        params={"name": player_1_name},
    )
    assert p1_resp.status_code == 201
    p1 = Player(id=p1_resp.json()["id"], name=player_1_name)

    p2_resp = await client.post(
        join_game_url,
        params={"name": player_2_name},
    )
    assert p2_resp.status_code == 201
    p2 = Player(id=p2_resp.json()["id"], name=player_2_name)

    async def next_question():
        resp_next_answer = await client.post(
            next_question_url,
            headers={"Authorization": auth_header},
        )
        assert resp_next_answer.status_code == 200

    await next_question()

    async def answer(player):
        resp = await client.post(
            request_answer_url,
            params={"player_id": player.id},
        )
        assert resp.status_code == 201
        assert resp.json() == {"answered": True, "error": None}

    await answer(p1)
    await answer(p2)

    async def check_answers_order():
        resp_answer_queue = await client.get(
            check_answer_order_url,
        )
        assert resp_answer_queue.status_code == 200
        return resp_answer_queue.json()["answers"]

    answers_order = await check_answers_order()

    assert answers_order[0]["player_id"] == p1.id
    assert answers_order[1]["player_id"] == p2.id

    await next_question()

    player_3_name = "Kolya"
    p3_resp = await client.post(
        join_game_url,
        params={"name": player_3_name},
    )
    assert p3_resp.status_code == 201
    p3 = Player(id=p3_resp.json()["id"], name=player_3_name)

    for p in [p3, p1, p2]:
        await answer(p)

    answers_order = await check_answers_order()
    assert answers_order[0]["player_id"] == p3.id
    assert answers_order[1]["player_id"] == p1.id
    assert answers_order[2]["player_id"] == p2.id

    await next_question()

    end_resp = await client.post(
        end_game_url,
        headers={"Authorization": auth_header},
    )

    assert end_resp.status_code == 200
    assert end_resp.json() == {"game": "ended"}
