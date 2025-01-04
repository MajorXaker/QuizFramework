from fastapi import APIRouter, status
from starlette.responses import JSONResponse, RedirectResponse

service_router = APIRouter()


@service_router.get("/healthcheck", tags=["healthcheck"])
async def healthcheck():
    status_code = status.HTTP_200_OK
    return JSONResponse({"status": "ok"}, status_code)
