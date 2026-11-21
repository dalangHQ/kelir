// see: cans/07-wire-plane.md#Userspace-network-stack
use super::*;
use kelir_interface::wire::DnsVerdict;

impl PipelineEngine {
    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn dispatch_net_outbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::NetOutbound)?;
        let remote = Self::net_remote_endpoint(frame, false);
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if !Self::net_scope_allows(&scope, remote) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // TODO: [plugin_engine] fill in the L3 wave; true preemption; the deadline is checked after the hook returns.; slop: overrunning hook holds the ring past its deadline.
            let deadline = self.net_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_net_outbound(frame),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut action = action;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                action = FilterAction::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                };
            }
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        drop(fused);
        Ok(FilterAction::Forward)
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    pub fn dispatch_net_inbound(&self, frame: &mut [u8]) -> Result<FilterAction, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::NetInbound)?;
        let remote = Self::net_remote_endpoint(frame, true);
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if !Self::net_scope_allows(&scope, remote) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_net_inbound(frame),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        drop(fused);
        Ok(FilterAction::Forward)
    }

    // see: cans/07-wire-plane.md#Hook-points:-on-net-outbound,-on-net-inbound,-on-socket-connect
    pub fn dispatch_socket_connect(
        &self,
        pid: u32,
        proto: u8,
        ip: [u8; 4],
        port: u16,
    ) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::SocketConnect)?;
        if let Some(verdict) = fused
            .as_ref()
            .and_then(|crossing| crossing.non_pass_verdict())
        {
            return Ok(verdict);
        }
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if !Self::net_scope_allows(&scope, Some((ip, port))) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // plugin_engine]: true preemption; the deadline is checked after the hook returns.; slop: overrunning hook holds the ring past its deadline.
            let deadline = self.net_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_socket_connect(pid, proto, ip, port),
                    || {
                        Ok(Verdict::Drop {
                            errno: Errno::Efault,
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = Verdict::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                };
            }
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }

    // see: cans/07-wire-plane.md#Synthetic-DNS-gateway
    pub fn dispatch_dns_query(
        &self,
        pid: u32,
        domain: &str,
        qtype: u16,
    ) -> Result<DnsVerdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::DnsQuery)?;
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // plugin_engine]: true preemption; the deadline is checked after the hook returns.; slop: overrunning hook holds the ring past its deadline.
            let deadline = self.net_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_dns_query(pid, domain, qtype),
                    || Ok(DnsVerdict::Sinkhole { rcode: 2 }),
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = DnsVerdict::Sinkhole { rcode: 2 };
            }
            if !matches!(verdict, DnsVerdict::Forward) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(DnsVerdict::Forward)
    }

    // see: cans/07-wire-plane.md#Additional-net-hooks:-on-dns-query,-on-socket-bind
    pub fn dispatch_socket_bind(
        &self,
        pid: u32,
        proto: u8,
        ip: [u8; 4],
        port: u16,
    ) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::SocketBind)?;
        if let Some(verdict) = fused
            .as_ref()
            .and_then(|crossing| crossing.non_pass_verdict())
        {
            return Ok(verdict);
        }
        for (index, interceptor) in self.net_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.net_scopes, index);
            if !scope.allows_device(VirtioDevice::Net) {
                continue;
            }
            if !Self::net_scope_allows(&scope, Some((ip, port))) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            // see: cans/02-plugin-model.md#Per-ring-timeouts
            // plugin_engine]: true preemption; the deadline is checked after the hook returns.; slop: overrunning hook holds the ring past its deadline.
            let deadline = self.net_regs.get(index).and_then(|reg| reg.timeout);
            let started = Instant::now();
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_socket_bind(pid, proto, ip, port),
                    || {
                        Ok(Verdict::Drop {
                            errno: Errno::Efault,
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            let mut verdict = verdict;
            if deadline.is_some_and(|limit| started.elapsed() > limit) {
                verdict = Verdict::Drop {
                    errno: Errno::Etimeout,
                    reason: "hook timeout".into(),
                };
            }
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    fn net_scope_allows(scope: &MountScope, remote: Option<([u8; 4], u16)>) -> bool {
        match remote {
            Some((ip, port)) => scope.allows_net(ip, port),
            None => scope.net_cidrs.is_none() && scope.net_ports.is_none(),
        }
    }

    // see: cans/02-plugin-model.md#Mount-scoping
    // see: cans/07-wire-plane.md#Userspace-network-stack
    fn net_remote_endpoint(frame: &[u8], inbound: bool) -> Option<([u8; 4], u16)> {
        use crate::planes::wire::layout_wire::{
            ETH_HEADER_BYTES, ETHERTYPE_IPV4, EthView, IP_PROTO_TCP, IP_PROTO_UDP,
            IPV4_MIN_HEADER_BYTES, Ipv4View, L4_PORT_PAIR_BYTES,
        };
        if frame.len() < ETH_HEADER_BYTES || EthView::new_unchecked(frame).etype() != ETHERTYPE_IPV4
        {
            return None;
        }
        let ip = Ipv4View::new_unchecked(&frame[ETH_HEADER_BYTES..]);
        let ihl = ip.ihl_bytes_checked()?;
        if ihl < IPV4_MIN_HEADER_BYTES
            || frame.len() < ETH_HEADER_BYTES + ihl + L4_PORT_PAIR_BYTES
            || !matches!(ip.proto(), IP_PROTO_TCP | IP_PROTO_UDP)
        {
            return None;
        }
        let l4 = &frame[ETH_HEADER_BYTES + ihl..];
        if inbound {
            Some((ip.src_checked()?, u16::from_be_bytes([l4[0], l4[1]])))
        } else {
            Some((ip.dst_checked()?, u16::from_be_bytes([l4[2], l4[3]])))
        }
    }
}
