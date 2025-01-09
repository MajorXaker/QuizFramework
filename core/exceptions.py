class UnknownPlayerError(ValueError):
    pass


class GameEnded(ValueError):
    pass


class JoiningRestrictedError(PermissionError):
    pass


class DuplicateValue(ValueError):
    pass
