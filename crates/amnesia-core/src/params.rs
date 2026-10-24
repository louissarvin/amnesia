/// Desk parameters. Times in seconds, values in zatoshis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    pub epoch_s: u64,
    pub grace_s: u64,
    pub burial_blocks: u32,
    /// Receive addresses pre-generated per epoch.
    pub cap: u32,
    /// Ops per hour on the desk-wide Poisson clock.
    pub merge_rate_per_h: u32,
    pub d_min: u64,
    pub carry_min: u64,
    pub carry_max: u64,
    pub max_canonical: usize,
    pub tx_expiry_delta: u32,
    /// Confirmations before a note counts as spendable for desk ops.
    pub min_conf: u32,
}

const TAZ: u64 = 100_000_000;

impl Params {
    /// Hosted demo profile (spec 18 §5.3).
    pub fn demo() -> Self {
        Self {
            epoch_s: 2 * 3600,
            grace_s: 2 * 3600,
            burial_blocks: 600,
            cap: 50,
            merge_rate_per_h: 12,
            d_min: TAZ / 1000,
            carry_min: TAZ / 200,
            carry_max: TAZ * 35 / 1000,
            max_canonical: 4,
            tx_expiry_delta: 40,
            min_conf: 10,
        }
    }

    /// Fast profile for local development only: never use for the hosted demo.
    pub fn dev() -> Self {
        Self {
            epoch_s: 15 * 60,
            grace_s: 15 * 60,
            burial_blocks: 20,
            cap: 10,
            merge_rate_per_h: 60,
            min_conf: 3,
            ..Self::demo()
        }
    }

    pub fn prod() -> Self {
        Self {
            epoch_s: 12 * 3600,
            grace_s: 12 * 3600,
            cap: 500,
            merge_rate_per_h: 30,
            ..Self::demo()
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "demo" => Some(Self::demo()),
            "dev" => Some(Self::dev()),
            "prod" => Some(Self::prod()),
            _ => None,
        }
    }
}
