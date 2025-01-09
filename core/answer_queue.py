from asyncio import Lock
from datetime import datetime, timezone

from core.exceptions import DuplicateValue
from models.player import Player
from models.registered_response import RegisteredResponse


class SimpleAnswerQueue:
    _answers: list[RegisteredResponse]
    _players_answered: set[Player]
    _queue_lock: Lock = Lock()
    _set_lock: Lock = Lock()

    async def flush(self):
        async with self._queue_lock, self._set_lock:
            self._answers = []
            self._players_answered = set()

    def __init__(self):
        self._answers = []
        self._players_answered = set()
        self._queue_lock = Lock()
        self._set_lock = Lock()

    async def register_answer(
        self, player, question_started: datetime
    ) -> RegisteredResponse:
        """
        Registers a player's answer by calculating the duration since the question started
        and storing it as a RegisteredResponse. Ensures that a player can only answer once.

        Args:
            player (Player): The player providing the answer.
            question_started (datetime): The timestamp when the question was started.

        Returns:
            RegisteredResponse: The response registered for the player's answer,
            including the time taken to answer.

        Raises:
            DuplicateValue: If the player has already answered the question.
        """
        async with self._set_lock:
            if player in self._players_answered:
                raise DuplicateValue("Player has already answered")
            async with self._queue_lock:
                self._players_answered.add(player)
                duration = (datetime.now(timezone.utc) - question_started).microseconds
                registered_response = RegisteredResponse.of_player(
                    player=player, duration=duration
                )
                self._answers.append(registered_response)
                return registered_response

    async def get_answers_order(self) -> list[RegisteredResponse]:
        async with self._queue_lock:
            return self._answers
