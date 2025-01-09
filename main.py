import uvicorn as uvicorn
from fastapi import FastAPI
from starlette.responses import RedirectResponse

from config import settings
from endpoints.admin import admin_router
from endpoints.healchcheck import service_router
from endpoints.html_endpoint import html_router
from endpoints.player import player_router

docs_conf = {"docs_url": None, "redoc_url": None, "openapi_url": None}
if settings.ENABLE_DOCS:
    docs_conf["docs_url"] = "/docs"
    docs_conf["redoc_url"] = "/redoc"
    docs_conf["openapi_url"] = "/openapi.json"

description = """
QuizHelper API
This app is a web app for live quizes, to verify who's the first to answer
It has two sides: admin and player
Admin side is for controlling the game and player side is for players to join and submit answers
"""

app = FastAPI(
    version="0.0.1",
    title=settings.PROJECT_NAME,
    summary="QuizHelper API",
    description=description,
    **docs_conf,
)


middlewares = [
    # AuthMiddleware(
    #     {
    #         "ES256": settings.JWT_PUBLIC_KEY,
    #         "HS256": settings.JWT_SECRET,
    #     }
    # ),
    # LogMiddleware(),
]
extensions = []


@app.get("/", include_in_schema=False)
def index():
    return RedirectResponse("/docs")


app.include_router(admin_router)
app.include_router(player_router)
app.include_router(service_router)
# app.include_router(html_router)

if __name__ == "__main__":
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=8000,
        log_level="info",
        reload=False,
        log_config=None,
    )
