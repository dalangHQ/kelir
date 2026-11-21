# see: cans/12-adapters.md#E2B-compatibility
from kelir import Sandbox as _CoreSandbox


class Sandbox:
    def __init__(self, inner: _CoreSandbox) -> None:
        self._inner = inner

    @classmethod
    async def create(cls) -> "Sandbox":
        inner = await (await _CoreSandbox.create()).boot()
        return cls(inner)

    async def close(self) -> None:
        await self._inner.stop()
