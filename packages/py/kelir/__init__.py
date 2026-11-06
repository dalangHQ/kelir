# see: cans/03-sdk-architecture.md#Python-SDK
from types import TracebackType
from typing import Self


class Session:
    async def rollback(self) -> None:
        """Rollback compute and storage to snapshot base."""
        # TODO: Call native driver rollback via PyO3 handle
        pass

    async def stop(self) -> None:
        """Gracefully terminate microVM session."""
        pass


class KelirBuilder:
    def __init__(self) -> None:
        self._plugins: list[object] = []
        self._session: Session | None = None

    def use(self, plugin: object) -> Self:
        self._plugins.append(plugin)
        return self

    async def boot(self) -> Session:
        # TODO: Validate Pydantic dependencies and boot via PyO3
        return Session()

    async def __aenter__(self) -> Session:
        self._session = await self.boot()
        return self._session

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        if self._session is not None:
            await self._session.stop()
            self._session = None


def kelir() -> KelirBuilder:
    return KelirBuilder()
