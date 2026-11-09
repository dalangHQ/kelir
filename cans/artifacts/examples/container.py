# examples/container.py
"""
Rootless OCI containers inside a microVM. The container plugin
configures a rootless Podman/crun environment, mounts an ephemeral
scratch device for /var/lib/containers, builds the synthetic cgroup
v2 delegation hierarchy at on_vm_ready, and pins subuid/subgid ranges
to unprivileged guest users; OCI images are ingested from pre-cached
host tarballs, with no nested VMX at any point. A privileged escape
attempt (raw device mount) is rejected with EPERM by the trap filter.

Mounts:     container.
Workload:   podman run of an ingested OCI image; the privileged
            attempt after it must fail.

Requirements:
    kelir>=1.0
"""

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

        # Container: values from the default profile
        # (fixtures/kelir.yaml#plugins.container). The profile disables
        # the plugin by default, so mounting it here enables it.
        # The plugin configures the rootless Podman/crun environment,
        # mounts the scratch device, builds the cgroup v2 delegation
        # hierarchy at on_vm_ready, and pins the id maps.
        .plugin(ContainerPlugin(ContainerConfig(
            runtime="crun",
            isolate_var_lib_containers=True,
            pin_subuid_ranges=True,
        )))
    )


async def main() -> None:
    async with build_runtime() as session:
        # Ingest the pre-cached OCI tarball from the host package cache
        # (plugins/container.md: no image translation, no nested VMX).
        image = await session.cap.container.ingest_image(OCI_TARBALL)

        run = await session.spawn(
            "podman", ["run", "--rm", "--rootless", image,
                       "echo", "hello from the microVM"],
        )
        if run.exit_code != 0:
            raise SystemExit(f"container run failed: {run.exit_code}")
        print(f"[kelir] container ok: {image}")

        # Privileged escape attempt: a raw device mount is rejected
        # with EPERM by the trap filter (plugins/container.md).
        escape = await session.spawn(
            "podman", ["run", "--rm", "--privileged",
                       "--device", "/dev/kvm", image, "true"],
        )
        if escape.exit_code == 0:
            raise SystemExit("privileged escape unexpectedly succeeded")
        print(f"[kelir] privileged attempt rejected: exit={escape.exit_code} (EPERM)")


if __name__ == "__main__":
    asyncio.run(main())
