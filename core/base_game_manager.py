from datetime import datetime, timezone

from models.player import Player
from models.registered_response import RegisteredResponse


class BaseGameManager:
    def __init__(self):
        self._players: dict[int, Player] = {}
        self._total_rounds = None
        self._current_round = None

        self._current_answer_order: list[RegisteredResponse] = []
        self._current_question_start: datetime = datetime.now(timezone.utc)

        self._joining_allowed = True

    def restart_game(self, rounds: int = None):
        """Method called when the game is restarted
        Args:
            rounds (int, optional): The number of rounds to play. Defaults to None.
        """
        raise NotImplementedError

    def add_player(self, player: Player):
        """Method called when a player joins the game
        Args:
            player (Player): The player who is joining the game.
        """
        raise NotImplementedError

    def end_game(self):
        """Method called when the game is over
        """
        raise NotImplementedError

    def request_answer(self, player):
        """Method called when players know answer and want to be registered
        Args:
            player (Player): The player who is requesting to answer.
        """
        raise NotImplementedError

    def next_round(self):
        """Method called when all players have submitted answers and want to move to the next round
        """
        raise NotImplementedError

    def allow_player_join(self):
        """Method to change the game state to allow players to join"""
        self._joining_allowed = True

    def prohibit_player_join(self):
        """Method to change the game state to prohibit players to join"""
        self._joining_allowed = False

    def allow_answer(self):
        """Method to change the game state to allow players to answer
        Answers before this method is called will be ignored"""
        raise NotImplementedError

    def get_players(self):
        return list(self._players.values())

    def see_response_order

