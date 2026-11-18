# see: cans/plugins/profile.md#Plugin-profile
# see: cans/plugins/profile.md#Declarative-profile-execution
# see: cans/02-plugin-model.md#Unified-builder
from __future__ import annotations

import asyncio

from kelir import KelirBuilder


async def main() -> None:
    builder = KelirBuilder.from_config("./fixtures/kelir.toml")
    builder.seal()
    async with builder as session:
        r = await session.spawn("git", ["status"])
        print(f"[kelir] git status -> exit={r.exit_code}")
        await session.export_spec("./fixtures/kelir.exported.toml")


if __name__ == "__main__":
    asyncio.run(main())
