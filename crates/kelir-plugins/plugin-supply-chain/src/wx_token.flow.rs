// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use kelir_interface::driver_interface::Stream;
use kelir_interface::plugin_model::PluginError;
use kelir_interface::vsock_mesh::VsockPeer;

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WxAllocToken {
    pub nonce: u128,
    pub issued_at_ns: u64,
    pub binary_blake3: [u8; 32],
}

impl WxAllocToken {
    // TODO (L3) [supply_chain_security]: cross-impl wire sync if a canonical token layout is pinned; see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation; slop: token layout unpinned, cross-impl wire sync diverges silently
    pub const WIRE_LEN: usize = 16 + 8 + 32;

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub fn wire_bytes(&self) -> [u8; Self::WIRE_LEN] {
        let mut wire = [0u8; Self::WIRE_LEN];
        wire[..16].copy_from_slice(&self.nonce.to_le_bytes());
        wire[16..24].copy_from_slice(&self.issued_at_ns.to_le_bytes());
        wire[24..].copy_from_slice(&self.binary_blake3);
        wire
    }
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
pub(crate) struct WxTokenLedger {
    next_nonce: AtomicU64,
    minted: Mutex<Vec<WxAllocToken>>,
}

impl WxTokenLedger {
    pub(crate) fn new() -> Self {
        Self {
            next_nonce: AtomicU64::new(1),
            minted: Mutex::new(Vec::new()),
        }
    }

    fn lock_minted(&self) -> Result<MutexGuard<'_, Vec<WxAllocToken>>, PluginError> {
        self.minted
            .lock()
            .map_err(|_| PluginError::Halt("W^X token ledger poisoned".into()))
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub(crate) fn mint(&self, binary_blake3: &[u8; 32]) -> Result<WxAllocToken, PluginError> {
        // TODO (L3) [supply_chain_security]: mint the nonce from the host CSPRNG and sign the token before vsock delivery; slop: nonce zeroed or unsigned, unverified binary presents valid-looking token
        let nonce = u128::from(self.next_nonce.fetch_add(1, Ordering::AcqRel));
        let issued_at_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| PluginError::Halt(format!("host clock before epoch: {e}")))?
            .as_nanos()
            .try_into()
            .map_err(|_| PluginError::Halt("host clock exceeds 64-bit nanoseconds".into()))?;
        let token = WxAllocToken {
            nonce,
            issued_at_ns,
            binary_blake3: *binary_blake3,
        };
        self.lock_minted()?.push(token.clone());
        Ok(token)
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub(crate) fn consume(&self, nonce: u128) -> Result<WxAllocToken, PluginError> {
        let mut minted = self.lock_minted()?;
        let index = minted
            .iter()
            .position(|token| token.nonce == nonce)
            .ok_or_else(|| {
                PluginError::Halt(format!(
                    "EWXVIOLATION: W^X allocation token {nonce:#x} is unknown or already consumed"
                ))
            })?;
        Ok(minted.remove(index))
    }
}

// see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
pub(crate) struct WxTokenChannel {
    peer: Mutex<Option<VsockPeer>>,
    stream: Mutex<Option<Box<dyn Stream>>>,
    pending: Mutex<Vec<WxAllocToken>>,
}

impl WxTokenChannel {
    pub(crate) fn new() -> Self {
        Self {
            peer: Mutex::new(None),
            stream: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        }
    }

    fn lock_opt<T>(slot: &Mutex<T>) -> Result<MutexGuard<'_, T>, PluginError> {
        slot.lock()
            .map_err(|_| PluginError::Halt("W^X token channel poisoned".into()))
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub(crate) fn register(
        &self,
        peer: VsockPeer,
        stream: Box<dyn Stream>,
    ) -> Result<(), PluginError> {
        *Self::lock_opt(&self.peer)? = Some(peer);
        *Self::lock_opt(&self.stream)? = Some(stream);
        self.flush_pending()
    }

    // see: cans/plugins/supply-chain.md#Dynamic-W^X-JIT-execution-mediation
    pub(crate) fn deliver(&self, token: WxAllocToken) -> Result<(), PluginError> {
        Self::lock_opt(&self.pending)?.push(token);
        self.flush_pending()
    }

    fn flush_pending(&self) -> Result<(), PluginError> {
        let mut pending = Self::lock_opt(&self.pending)?;
        if pending.is_empty() {
            return Ok(());
        }
        let mut stream = Self::lock_opt(&self.stream)?;
        let Some(stream) = stream.as_mut() else {
            return Ok(());
        };
        // TODO (L3) [supply_chain_security]: correlate the channel with the trapped task and move token delivery onto the async transport loop; slop: token delivered off async loop, channel uncorrelated with trapped task
        for token in pending.iter() {
            let written = stream.write(&token.wire_bytes())?;
            if written != WxAllocToken::WIRE_LEN {
                return Err(PluginError::Halt(format!(
                    "W^X token vsock delivery wrote {written} of {} bytes",
                    WxAllocToken::WIRE_LEN
                )));
            }
        }
        pending.clear();
        Ok(())
    }
}
