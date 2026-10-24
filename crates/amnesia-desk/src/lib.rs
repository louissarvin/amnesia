//! amnesia-desk: a Zcash donation desk that forgets. Testnet only.

pub mod chain;
pub mod desk;
pub mod epoch;
pub mod hardening;
pub mod intake;
pub mod ledger;
pub mod log;
pub mod wallet;

use zcash_protocol::consensus::Network;

pub const PARAMS: Network = Network::TestNetwork;
