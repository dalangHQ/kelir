// see: cans/02-plugin-model.md#Mount-scoping
use std::path::{Path, PathBuf};

use crate::plugin_model::VirtioDevice;
use crate::protocol_wire::StreamKind;

// see: cans/artifacts/code/plugin-trait.rs#MountScope
// see: cans/02-plugin-model.md#Mount-scoping
#[derive(Debug, Clone, Default)]
pub struct MountScope {
    pub devices: Option<Vec<VirtioDevice>>,
    pub binaries: Option<Vec<String>>,
    pub paths: Option<Vec<PathBuf>>,
    pub stream_kinds: Option<Vec<StreamKind>>,
    // see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
    pub net_cidrs: Option<Vec<String>>,
    pub net_ports: Option<Vec<u16>>,
}

impl MountScope {
    pub fn with_devices<I: IntoIterator<Item = VirtioDevice>>(mut self, devices: I) -> Self {
        self.devices = Some(devices.into_iter().collect());
        self
    }

    pub fn with_binaries<S: Into<String>>(mut self, binaries: impl IntoIterator<Item = S>) -> Self {
        self.binaries = Some(binaries.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_paths<I: IntoIterator<Item = P>, P: Into<PathBuf>>(mut self, paths: I) -> Self {
        self.paths = Some(paths.into_iter().map(Into::into).collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn with_stream_kinds<I: IntoIterator<Item = StreamKind>>(mut self, kinds: I) -> Self {
        self.stream_kinds = Some(kinds.into_iter().collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn with_net_cidrs<S: Into<String>>(mut self, cidrs: impl IntoIterator<Item = S>) -> Self {
        self.net_cidrs = Some(cidrs.into_iter().map(Into::into).collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn with_net_ports<I: IntoIterator<Item = u16>>(mut self, ports: I) -> Self {
        self.net_ports = Some(ports.into_iter().collect());
        self
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    fn intersect_list<T: PartialEq + Clone>(
        child: &Option<Vec<T>>,
        parent: &Option<Vec<T>>,
    ) -> Option<Vec<T>> {
        match (child, parent) {
            (Some(a), Some(b)) => Some(a.iter().filter(|x| b.contains(x)).cloned().collect()),
            (Some(a), None) | (None, Some(a)) => Some(a.clone()),
            (None, None) => None,
        }
    }

    pub fn intersect(&self, parent: &Self) -> Self {
        Self {
            devices: Self::intersect_list(&self.devices, &parent.devices),
            binaries: Self::intersect_list(&self.binaries, &parent.binaries),
            paths: Self::intersect_list(&self.paths, &parent.paths),
            stream_kinds: Self::intersect_list(&self.stream_kinds, &parent.stream_kinds),
            net_cidrs: Self::intersect_list(&self.net_cidrs, &parent.net_cidrs),
            net_ports: Self::intersect_list(&self.net_ports, &parent.net_ports),
        }
    }

    // TODO (L3) [pe_20_same_path_different_identity_never_matches]: canonical (st_dev, st_ino, binary_blake3) — cans/11-protocol.md#Trap-fast-path; slop: swapped identity Passes, exact dev/ino/BLAKE3 match skipped
    // see: cans/02-plugin-model.md#Filter-before-FFI-gate
    pub fn allows_binary(&self, binary: &str) -> bool {
        self.binaries.as_ref().is_none_or(|list| {
            list.iter().any(|entry| {
                entry == binary
                    || (!entry.contains('/') && binary.rsplit('/').next() == Some(entry.as_str()))
            })
        })
    }

    pub fn allows_path(&self, path: &Path) -> bool {
        self.paths
            .as_ref()
            .is_none_or(|list| list.iter().any(|item| path.starts_with(item)))
    }

    pub fn allows_device(&self, device: VirtioDevice) -> bool {
        self.devices
            .as_ref()
            .is_none_or(|list| list.contains(&device))
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn allows_stream_kind(&self, kind: StreamKind) -> bool {
        self.stream_kinds
            .as_ref()
            .is_none_or(|list| list.contains(&kind))
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn allows_net(&self, ip: [u8; 4], port: u16) -> bool {
        let cidr_ok = self
            .net_cidrs
            .as_ref()
            .is_none_or(|list| list.is_empty() || list.iter().any(|cidr| cidr_contains(cidr, ip)));
        let port_ok = self
            .net_ports
            .as_ref()
            .is_none_or(|list| list.is_empty() || list.contains(&port));
        cidr_ok && port_ok
    }
}

// see: cans/02-plugin-model.md#Mount-scoping
fn cidr_contains(cidr: &str, ip: [u8; 4]) -> bool {
    let Some((addr, prefix)) = cidr.split_once('/') else {
        return false;
    };
    let (Ok(prefix), Some(network)) = (prefix.parse::<u32>(), parse_ipv4(addr)) else {
        return false;
    };
    if prefix > 32 {
        return false;
    }
    let mask = u32::MAX.checked_shl(32 - prefix).unwrap_or(0);
    (u32::from_be_bytes(ip) & mask) == (network & mask)
}

// see: cans/02-plugin-model.md#Mount-scoping
fn parse_ipv4(addr: &str) -> Option<u32> {
    let mut parts = addr.split('.');
    let mut octets = [0u8; 4];
    for slot in &mut octets {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(u32::from_be_bytes(octets))
}
