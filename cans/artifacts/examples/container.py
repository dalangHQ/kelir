# see: cans/plugins/container.md#Plugin-container
# see: cans/plugins/container.md#Non-nested-OCI-container-execution
# see: cans/plugins/container.md#Rootless-container-execution
# see: cans/01-architecture.md#VirtIO-Device-Complex
from __future__ import annotations

import asyncio
import os

import kelir
from kelir import KelirBuilder, Session
from kelir.plugins.plugin_container import ContainerConfig, ContainerPlugin


OCI_TARBALL = "./fixtures/oci-hello-world.tar"


def build_runtime() -> KelirBuilder:
    return (
        kelir.builder(
            vcpus=2, memory_mb=2048, driver="auto",
            kernel=f"./fixtures/vmlinux-6.6-{os.uname().machine}-kelir",
            rootfs_erofs="./fixtures/rootfs-container.erofs",
            tsc_locked=True, enable_watchdog=True,
        )

        .use(ContainerPlugin(ContainerConfig(
            runtime="crun",
            isolate_var_lib_containers=True,
            pin_subuid_ranges=True,
        )))
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        image = await session.cap.container.ingest_image(OCI_TARBALL)

        run = await session.spawn(
            "podman", ["run", "--rm", "--rootless", image,
                       "echo", "hello from the microVM"],
        )
        if run.exit_code != 0:
            raise SystemExit(f"container run failed: {run.exit_code}")
        print(f"[kelir] container ok: {image}")

        # see: cans/plugins/container.md#Privileged-container-escape-attempt-or-raw-device-mount-rejects-with-EPERM
        escape = await session.spawn(
            "podman", ["run", "--rm", "--privileged",
                       "--device", "/dev/kvm", image, "true"],
        )
        if escape.exit_code == 0:
            raise SystemExit("privileged escape unexpectedly succeeded")
        print(f"[kelir] privileged attempt rejected: exit={escape.exit_code} (EPERM)")


if __name__ == "__main__":
    asyncio.run(main())
