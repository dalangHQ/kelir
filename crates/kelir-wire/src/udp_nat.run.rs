// see: cans/07-wire-plane.md#Wire-architecture
// see: cans/07-wire-plane.md#UDP-socket-reaper
use std::sync::{Arc, Mutex};

use kelir_interface::plugin_model::PluginError;
use kelir_interface::wire::udp_nat::{
    UDP_NAT_IDLE_TIMEOUT_SECS, UDP_NAT_SLAB_CAPACITY, UdpNatEntry, UdpNatOps,
};

pub(crate) struct RealUdpNatOps {
    udp_nat_slab: Mutex<Vec<Option<UdpNatEntry>>>,
}

impl RealUdpNatOps {
    pub(crate) fn new() -> Self {
        Self {
            udp_nat_slab: Mutex::new(Vec::new()),
        }
    }
}

impl Default for RealUdpNatOps {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn build_udp_nat_ops() -> Arc<dyn UdpNatOps> {
    Arc::new(RealUdpNatOps::new())
}

impl UdpNatOps for RealUdpNatOps {
    // see: cans/07-wire-plane.md#UDP-socket-reaper
    fn udp_nat_alloc(&self, guest_source_port: u16, now_unix: u64) -> Result<(), PluginError> {
        let mut slab = self
            .udp_nat_slab
            .lock()
            .map_err(|_| PluginError::Halt("udp nat slab lock poisoned".into()))?;
        if let Some(slot) = slab
            .iter_mut()
            .flatten()
            .find(|entry| entry.guest_source_port() == guest_source_port)
        {
            slot.touch(now_unix);
            return Ok(());
        }
        // TODO (L3) [wire_plane]: open the per-source-port host UDP socket and store its descriptor in the slab entry (Full Cone endpoint independence); slop: slab entry without host socket, endpoint independence broken
        let entry = UdpNatEntry::new(guest_source_port, now_unix);
        if let Some(free) = slab.iter().position(Option::is_none) {
            slab[free] = Some(entry);
            return Ok(());
        }
        if slab.len() < UDP_NAT_SLAB_CAPACITY {
            slab.push(Some(entry));
            return Ok(());
        }
        let oldest = slab.iter().enumerate().min_by_key(|(_, slot)| {
            slot.as_ref()
                .map_or(u64::MAX, |entry| entry.last_active_unix())
        });
        let Some((index, _)) = oldest else {
            return Err(PluginError::Halt("udp nat slab cap exhausted".into()));
        };
        slab[index] = Some(entry);
        Ok(())
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    fn udp_nat_slab_used(&self) -> Result<usize, PluginError> {
        let slab = self
            .udp_nat_slab
            .lock()
            .map_err(|_| PluginError::Halt("udp nat slab lock poisoned".into()))?;
        Ok(slab.iter().flatten().count())
    }

    // see: cans/07-wire-plane.md#UDP-socket-reaper
    fn reap_stale_udp_sockets(&self, now_unix: u64) -> Result<usize, PluginError> {
        let mut slab = self
            .udp_nat_slab
            .lock()
            .map_err(|_| PluginError::Halt("udp nat slab lock poisoned".into()))?;
        let mut reaped = 0usize;
        for slot in slab.iter_mut() {
            let Some(entry) = slot.as_ref() else {
                continue;
            };
            if now_unix.saturating_sub(entry.last_active_unix()) > UDP_NAT_IDLE_TIMEOUT_SECS {
                // TODO (L3) [wire_plane]: close(2) the host UDP socket descriptor before releasing the slab slot; slop: slab released with host fd open, descriptor leaks
                *slot = None;
                reaped += 1;
            }
        }
        Ok(reaped)
    }

    fn quiesce(&self) -> Result<(), PluginError> {
        let mut slab = self
            .udp_nat_slab
            .lock()
            .map_err(|_| PluginError::Halt("udp nat slab lock poisoned".into()))?;
        slab.clear();
        Ok(())
    }
}
