# see: cans/12-adapters.md#E2B-compatibility
from kelir import Session, kelir


class Sandbox:
    def __init__(self, session: Session) -> None:
        self._session = session

    @classmethod
    async def create(cls) -> "Sandbox":
        session = await kelir().boot()
        return cls(session)

    async def close(self) -> None:
        await self._session.stop()
