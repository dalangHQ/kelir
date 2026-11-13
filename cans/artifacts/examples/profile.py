# see: cans/plugins/profile.md#Plugin-profile
# see: cans/plugins/profile.md#Declarative-profile-execution
from __future__ import annotations

import asyncio

from kelir import Sandbox


async def main() -> None:
    box = await Sandbox.from_config("./fixtures/kelir.toml")
    await box.boot()

    r = await box.exec("git", ["status"])
    print(f"[kelir] git status -> exit={r.exit_code}")

    await box.export_spec("./fixtures/kelir.exported.toml")

    await box.terminate()


if __name__ == "__main__":
    asyncio.run(main())
