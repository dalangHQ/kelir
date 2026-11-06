// see: cans/02-plugin-model.md#Catalog-tiering
// TODO (L2): add modules for the remaining catalog plugins as their SDK surfaces land per cans/plugins/*.md.
#[cfg(feature = "plugin-chaos")]
pub mod chaos;
#[cfg(feature = "plugin-ledger")]
pub mod ledger;
#[cfg(feature = "plugin-mesh")]
pub mod mesh;
#[cfg(feature = "plugin-replay")]
pub mod replay;
#[cfg(feature = "plugin-supply-chain")]
pub mod supply_chain;
