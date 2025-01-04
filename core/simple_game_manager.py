from datetime import datetime, timezone

from core.base_game_manager import BaseGameManager
from models.player import Player
from models.registered_response import RegisteredResponse


class SimpleGameManager(BaseGameManager):
    def restart_game(self, rounds: int = None):
        self._players = {}

    def add_player(self, player: Player):
        self._players[player.id] = player

    def end_game(self):
        self._players = {}

    def request_answer(self, player):
        """Method called when players know answer and want to be registered
        Args:
            player (Player): The player who is requesting to answer.
        """
        duration = (datetime.now(timezone.utc) - self._current_question_start).microseconds
        registered_response = RegisteredResponse(player_id=player.id, time_to_answer_ms=duration)
        self._current_answer_order.append(registered_response)
        return registered_response

    def next_round(self):
        self._current_answer_order = []
        if self._total_rounds:
            self._current_round += 1

    def get_my_answer_order(self, player: Player):
        ...






game = SimpleGameManager()