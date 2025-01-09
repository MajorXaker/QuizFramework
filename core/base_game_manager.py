from datetime import datetime, timezone

from models.player import Player
from models.registered_response import RegisteredResponse


class BaseGameManager:
    def __init__(self):
        pass

    async def restart_game(self, rounds: int = None):
        """Method called when the game is restarted
        Args:
            rounds (int, optional): The number of rounds to play. Defaults to None.
        """
        pass

    async def add_player(self, player: Player):
        """Method called when a player joins the game
        Args:
            player (Player): The player who is joining the game.
        """
        ...

    async def end_game(self) -> None:
        """Method called when the game is over"""
        ...

    async def request_answer(self, player: Player) -> RegisteredResponse:
        """Method to register a player's answer

        Args:
            player (Player): The player who is requesting to answer.

        Returns:
            RegisteredResponse: The response registered for the player's answer, including the time taken to answer.
        """
        ...

    async def next_round(self) -> None:
        """Method called when all players have submitted answers and want to move to the next round"""
        ...

    async def allow_player_join(self) -> None:
        """Method to change the game state to allow players to join"""
        ...

    async def prohibit_player_join(self) -> None:
        """Method to change the game state to prohibit players to join"""
        ...

    async def get_players(self) -> list[Player]:
        """
        Method to get all players instances
        :return: list of Player objects
        """
        ...

    async def get_player_by_id(self, player_id: int) -> Player:
        """
        Method to retrieve a player by their unique ID.

        Args:
            player_id (int): The unique identifier of the player to retrieve.

        Returns:
            Player: The player object corresponding to the given ID.

        Raises:
            KeyError: If no player with the specified ID is found.
        """
        ...

    async def get_answers_order(self) -> list[RegisteredResponse]:
        """
        Method to get the order in which players submitted their answers.

        Returns:
            list[RegisteredResponse]: The list of RegisteredResponse objects.
        """
        ...
