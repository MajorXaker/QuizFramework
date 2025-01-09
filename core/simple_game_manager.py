from datetime import datetime, timezone
from threading import Lock

from core.answer_queue import SimpleAnswerQueue
from core.base_game_manager import BaseGameManager
from core.exceptions import (
    UnknownPlayerError,
    GameEnded,
    JoiningRestrictedError,
    DuplicateValue,
)
from models.player import Player
from models.registered_response import RegisteredResponse


class SingletonMeta(type):
    """
    This is a thread-safe implementation of Singleton.
    """

    _instances = {}

    _lock: Lock = Lock()
    """
    We now have a lock object that will be used to synchronize threads during
    first access to the Singleton.
    """

    def __call__(cls, *args, **kwargs):
        """
        Possible changes to the value of the `__init__` argument do not affect
        the returned instance.
        """
        # Now, imagine that the program has just been launched. Since there's no
        # Singleton instance yet, multiple threads can simultaneously pass the
        # previous conditional and reach this point almost at the same time. The
        # first of them will acquire lock and will proceed further, while the
        # rest will wait here.
        with cls._lock:
            # The first thread to acquire the lock, reaches this conditional,
            # goes inside and creates the Singleton instance. Once it leaves the
            # lock block, a thread that might have been waiting for the lock
            # release may then enter this section. But since the Singleton field
            # is already initialized, the thread won't create a new object.
            if cls not in cls._instances:
                instance = super().__call__(*args, **kwargs)
                cls._instances[cls] = instance
        return cls._instances[cls]


class SimpleGameManager(BaseGameManager, metaclass=SingletonMeta):
    is_live: bool

    def __init__(self):
        super().__init__()
        self.is_live = False

        self._current_answer_queue: SimpleAnswerQueue = SimpleAnswerQueue()
        self._current_question_start: datetime = datetime.now(timezone.utc)

        self._players: dict[int, Player] = {}
        self._total_rounds = None
        self._current_round = None

        self._joining_allowed = True

    async def restart_game(self, rounds: int = None):
        await self._current_answer_queue.flush()
        self._players = {}
        self._total_rounds = rounds
        self._current_round = 0
        self.is_live = True
        self._reset_counter()
        await super().restart_game(rounds)

    def _reset_counter(self):
        """
        Method called when a new round starts
        :return:
        """
        self._current_question_start = datetime.now(timezone.utc)

    async def add_player(self, player: Player):
        if player.id in self._players:
            raise DuplicateValue("Player already exists")
        if not self.is_live:
            raise GameEnded("Game is not live")
        if not self._joining_allowed:
            raise JoiningRestrictedError("Joining is not allowed")
        self._players[player.id] = player

    async def end_game(self):
        self._players = {}
        self.is_live = False

    async def request_answer(self, player):
        if player.id not in self._players:
            raise UnknownPlayerError
        if self._current_round == 0:
            raise GameEnded

        registered_response = await self._current_answer_queue.register_answer(
            player=player,
            question_started=self._current_question_start,
        )
        return registered_response

    async def next_round(self):
        await self._current_answer_queue.flush()
        self._current_round += 1
        if self._total_rounds and self._current_round > self._total_rounds:
            await self.end_game()
            raise GameEnded

    async def get_players(self):
        return list(self._players.values())

    async def get_player_by_id(self, player_id: int):
        try:
            return self._players[player_id]
        except KeyError:
            raise UnknownPlayerError

    async def allow_player_join(self):
        self._joining_allowed = True

    async def prohibit_player_join(self):
        self._joining_allowed = False

    async def get_answers_order(self) -> list[RegisteredResponse]:
        return await self._current_answer_queue.get_answers_order()


game_manager = SimpleGameManager()
