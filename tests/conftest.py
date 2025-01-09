import asyncio

import pytest
from httpx import AsyncClient, ASGITransport
from pytest_mock import MockerFixture

from core.simple_game_manager import SimpleGameManager
from endpoints import auth
from main import app

# db_url = (
#     "postgresql+asyncpg://"
#     f"{st.DATABASE_USER}:"
#     f"{st.DATABASE_PASSWORD}@"
#     f"{st.DATABASE_HOST}:"
#     f"{st.DATABASE_PORT}/"
#     f"{st.DATABASE_DB}"
# )


@pytest.fixture(scope="session")
def event_loop():
    loop = asyncio.get_event_loop()
    yield loop
    loop.close()


# def check_test_db():
#     if (
#         st.DATABASE_HOST not in ("localhost", "127.0.0.1", "postgres")
#         or "amazonaws" in st.DATABASE_HOST
#     ):
#         print(db_url)
#         raise Exception("Use local database only!")


# @pytest.fixture(scope="session")
# async def engine():
#     check_test_db()
#
#     e = create_async_engine(db_url, echo=False, max_overflow=25)
#
#     try:
#         async with e.begin() as con:
#             await con.run_sync(m.Model.metadata.create_all)
#
#         yield e
#     finally:
#         async with e.begin() as con:
#             await con.run_sync(m.Model.metadata.drop_all)
# @pytest.fixture
# async def dbsession(engine) -> AsyncSession:
#     async with AsyncSession(bind=engine) as session:
#         yield session


@pytest.fixture
def patch_auth(mocker: MockerFixture):
    def inner(authenticated=False):
        if authenticated:
            mocker.patch.object(
                auth,
                "auth",
                return_value=None,
            )

    return inner


@pytest.fixture
async def client() -> AsyncClient:
    # def override_get_db():
    #     test_db = dbsession
    #     yield test_db
    #
    # app.dependency_overrides[get_session_dep.dependency] = override_get_db

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as ac:
        yield ac


@pytest.fixture
async def live_game_manager():
    game = SimpleGameManager()
    await game.restart_game()
    yield game
    game.end_game()
