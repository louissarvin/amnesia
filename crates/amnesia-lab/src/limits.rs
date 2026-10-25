//! Rate limits for the public demo API: global and per-client cooldowns, and a rolling-hour cap
//! on lab donations so the lab cannot fill a desk epoch by itself (n-1 attack).

use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv6Addr},
    time::{Duration, Instant},
};

use axum::http::HeaderMap;

/// Client cooldown entries kept before the oldest are evicted.
pub const MAX_CLIENTS: usize = 4096;
/// Lab donations (judges and bots combined) allowed per rolling hour.
pub const DONATIONS_PER_HOUR: usize = 6;
/// Longest client cooldown; older entries can be dropped without changing any decision.
const LONGEST_COOLDOWN: Duration = Duration::from_secs(20);

/// Bucket shared by every request that arrives without a usable proxy-appended `X-Forwarded-For` entry.
const NO_CLIENT: &str = "global";

/// Rate limit key for a request: the LAST X-Forwarded-For entry, the one our own reverse
/// proxy (Tailscale Funnel) appends. Earlier entries and other IP headers are client-supplied
/// and would let one client rotate keys; without the header every request shares one bucket.
pub fn client_key(h: &HeaderMap) -> String {
    let ip = h
        .get_all("x-forwarded-for")
        .iter()
        .last()
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.rsplit(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok());
    match ip {
        Some(IpAddr::V4(v4)) => v4.to_string(),
        // One IPv6 host usually owns a whole /64, so keying on the full address is free to dodge.
        Some(IpAddr::V6(v6)) => Ipv6Addr::from(u128::from(v6) & !((1u128 << 64) - 1)).to_string(),
        None => NO_CLIENT.to_string(),
    }
}

/// Rolling-window counter.
pub struct WindowCap {
    max: usize,
    window: Duration,
    hits: VecDeque<Instant>,
}

impl WindowCap {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            max,
            window,
            hits: VecDeque::new(),
        }
    }

    /// True if one more hit at `now` stays within the cap. Does not record it.
    pub fn allows(&mut self, now: Instant) -> bool {
        while self
            .hits
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) >= self.window)
        {
            self.hits.pop_front();
        }
        self.hits.len() < self.max
    }

    pub fn record(&mut self, now: Instant) {
        self.hits.push_back(now);
    }

    pub fn try_take(&mut self, now: Instant) -> bool {
        let ok = self.allows(now);
        if ok {
            self.record(now);
        }
        ok
    }
}

/// One admission rule: a global cooldown, a per-client cooldown, and optionally the donation cap.
pub struct Rule {
    pub global: &'static str,
    pub global_secs: u64,
    pub client: &'static str,
    pub client_secs: u64,
    pub donation: bool,
}

pub struct Limits {
    global: HashMap<&'static str, Instant>,
    clients: HashMap<String, Instant>,
    pub donations: WindowCap,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            global: HashMap::new(),
            clients: HashMap::new(),
            donations: WindowCap::new(DONATIONS_PER_HOUR, Duration::from_secs(3600)),
        }
    }
}

impl Limits {
    /// Checks every limit of `rule` and commits all of them only if all pass, so a rejection by
    /// one limit never burns another (a cooldown miss must not use up a donation slot).
    pub fn admit(&mut self, now: Instant, rule: &Rule, client: &str) -> Result<(), &'static str> {
        let ready = |t: Option<&Instant>, secs: u64| {
            t.is_none_or(|t| now.saturating_duration_since(*t) >= Duration::from_secs(secs))
        };
        let ckey = format!("{}/{client}", rule.client);
        if !ready(self.global.get(rule.global), rule.global_secs)
            || !ready(self.clients.get(&ckey), rule.client_secs)
        {
            return Err("cooldown");
        }
        if rule.donation && !self.donations.allows(now) {
            return Err("donation_cap_reached");
        }
        self.global.insert(rule.global, now);
        self.clients.insert(ckey, now);
        if rule.donation {
            self.donations.record(now);
        }
        if self.clients.len() > MAX_CLIENTS {
            self.clients
                .retain(|_, t| now.saturating_duration_since(*t) < LONGEST_COOLDOWN);
            evict_oldest(&mut self.clients, MAX_CLIENTS);
        }
        Ok(())
    }
}

fn evict_oldest(m: &mut HashMap<String, Instant>, max: usize) {
    while m.len() > max {
        let Some(k) = m.iter().min_by_key(|(_, t)| **t).map(|(k, _)| k.clone()) else {
            break;
        };
        m.remove(&k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DONATE: Rule = Rule {
        global: "donate",
        global_secs: 0,
        client: "donate",
        client_secs: 20,
        donation: true,
    };

    #[test]
    fn rolling_hour_cap() {
        let t0 = Instant::now();
        let mut cap = WindowCap::new(6, Duration::from_secs(3600));
        for i in 0..6 {
            assert!(cap.try_take(t0 + Duration::from_secs(i * 60)), "hit {i}");
        }
        assert!(!cap.try_take(t0 + Duration::from_secs(3599)));
        // The first hit ages out at exactly one hour; only one slot frees up.
        assert!(cap.try_take(t0 + Duration::from_secs(3600)));
        assert!(!cap.try_take(t0 + Duration::from_secs(3601)));
        assert!(cap.try_take(t0 + Duration::from_secs(3660)));
    }

    #[test]
    fn donation_cap_spans_clients_and_rejections_burn_nothing() {
        let t0 = Instant::now();
        let mut l = Limits::default();
        for i in 0..6 {
            assert_eq!(l.admit(t0, &DONATE, &format!("10.0.0.{i}")), Ok(()));
        }
        assert_eq!(
            l.admit(t0, &DONATE, "10.0.0.99"),
            Err("donation_cap_reached")
        );
        // A capped request must not have started a cooldown for that client.
        let later = t0 + Duration::from_secs(3600);
        assert_eq!(l.admit(later, &DONATE, "10.0.0.99"), Ok(()));
    }

    #[test]
    fn per_client_and_global_cooldowns() {
        let t0 = Instant::now();
        let seize = Rule {
            global: "seize_a",
            global_secs: 15,
            client: "seize",
            client_secs: 15,
            donation: false,
        };
        let mut l = Limits::default();
        assert_eq!(l.admit(t0, &seize, "a"), Ok(()));
        assert_eq!(
            l.admit(t0 + Duration::from_secs(5), &seize, "b"),
            Err("cooldown")
        );
        assert_eq!(l.admit(t0 + Duration::from_secs(15), &seize, "b"), Ok(()));
        let other = Rule {
            global: "seize_b",
            ..seize
        };
        assert_eq!(
            l.admit(t0 + Duration::from_secs(16), &other, "b"),
            Err("cooldown")
        );
        assert_eq!(l.admit(t0 + Duration::from_secs(16), &other, "a"), Ok(()));
    }

    #[test]
    fn client_map_is_bounded() {
        let t0 = Instant::now();
        let mut l = Limits::default();
        let rule = Rule {
            global: "x",
            global_secs: 0,
            client: "x",
            client_secs: 600,
            donation: false,
        };
        for i in 0..(MAX_CLIENTS + 10) {
            assert_eq!(
                l.admit(t0 + Duration::from_millis(i as u64), &rule, &i.to_string()),
                Ok(())
            );
        }
        assert_eq!(l.clients.len(), MAX_CLIENTS);
        assert!(!l.clients.contains_key("x/0"));
        assert!(l.clients.contains_key(&format!("x/{}", MAX_CLIENTS + 9)));
    }

    #[test]
    fn client_key_from_header() {
        let mut h = HeaderMap::new();
        assert_eq!(client_key(&h), "global");
        h.insert("x-forwarded-for", "not an ip".parse().unwrap());
        assert_eq!(client_key(&h), "global");
        h.insert("x-forwarded-for", " 203.0.113.7 ".parse().unwrap());
        assert_eq!(client_key(&h), "203.0.113.7");
        h.insert("x-forwarded-for", "2001:db8:1:2:aaaa::1".parse().unwrap());
        assert_eq!(client_key(&h), "2001:db8:1:2::");
    }

    #[test]
    fn client_key_uses_the_proxy_appended_entry() {
        let mut h = HeaderMap::new();
        assert_eq!(client_key(&h), NO_CLIENT);
        h.insert("cf-connecting-ip", "9.9.9.9".parse().unwrap());
        assert_eq!(client_key(&h), NO_CLIENT, "client-set headers are ignored");
        h.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());
        assert_eq!(client_key(&h), "5.6.7.8");
    }
}
