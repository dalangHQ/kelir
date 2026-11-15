// see: cans/plugins/container.md#Plugin-container
// see: cans/plugins/container.md#Non-nested-OCI-container-execution
// see: cans/plugins/container.md#Rootless-container-execution
// see: cans/01-architecture.md#VirtIO-Device-Complex

import { kelir, container, type ContainerHandle } from "@kelir/core";

const OCI_TARBALL = "./fixtures/oci-hello-world.tar";

const runtime = kelir({
  vcpus: 2, memoryMb: 2048, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-container.erofs",
  tscLocked: true, enableWatchdog: true,
})
  .use(container({
    runtime: "crun",
    isolateVarLibContainers: true,
    pinSubuidRanges: true,
  }));

async function main() {
  runtime.seal();
  const session = await runtime.boot();
  try {
    const containerCap = session.cap("container") as ContainerHandle;
    if (!containerCap) throw new Error("container capability handle not found");

    const image = await containerCap.ingestImage(OCI_TARBALL);

    const run = await session.spawn("podman",
      ["run", "--rm", "--rootless", image, "echo", "hello from the microVM"]);
    if (run.exitCode !== 0) throw new Error(`container run failed: ${run.exitCode}`);
    console.log(`[kelir] container ok: ${image}`);

    // see: cans/plugins/container.md#Privileged-container-escape-attempt-or-raw-device-mount-rejects-with-EPERM
    const escape = await session.spawn("podman",
      ["run", "--rm", "--privileged", "--device", "/dev/kvm", image, "true"]);
    if (escape.exitCode === 0) throw new Error("privileged escape unexpectedly succeeded");
    console.log(`[kelir] privileged attempt rejected: exit=${escape.exitCode} (EPERM)`);
  } finally {
    await session.terminate();
  }
}

main().catch(console.error);
