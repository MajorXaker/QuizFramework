import uvicorn as uvicorn

from fastapi import FastAPI
from starlette.middleware.cors import CORSMiddleware
from starlette.responses import RedirectResponse

from config import log, sentry_sdk, settings  # noqa: F401
from endpoints.admin import admin_router
from endpoints.healchcheck import service_router
from endpoints.player import player_router

docs_conf = {"docs_url": None, "redoc_url": None, "openapi_url": None}
if settings.ENABLE_DOCS:
    docs_conf["docs_url"] = "/docs"
    docs_conf["redoc_url"] = "/redoc"
    docs_conf["openapi_url"] = "/openapi.json"

app = FastAPI(
    version="0.0.1",
    title=settings.PROJECT_NAME,
    description="QuizHelper API",
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



app.include_router(admin_router)
app.include_router(player_router)
app.include_router(service_router)


if __name__ == "__main__":
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=8000,
        log_level="info",
        reload=False,
        log_config=None,
    )
