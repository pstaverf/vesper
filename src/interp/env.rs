use std::time::{SystemTime, UNIX_EPOCH};

pub trait EnvCtx {
    fn now(&mut self) -> i64;
    fn rand(&mut self) -> f64;
}

pub struct RealEnv {
    state: u64,
}

impl RealEnv {
    pub fn new() -> RealEnv {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        RealEnv { state: seed | 1 }
    }
}

impl Default for RealEnv {
    fn default() -> RealEnv {
        RealEnv::new()
    }
}

impl EnvCtx for RealEnv {
    fn now(&mut self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn rand(&mut self) -> f64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((v >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

pub struct FixedEnv {
    pub clock: i64,
    pub seq: Vec<f64>,
    pos: usize,
}

impl FixedEnv {
    pub fn new(clock: i64, seq: Vec<f64>) -> FixedEnv {
        FixedEnv { clock, seq, pos: 0 }
    }
}

impl EnvCtx for FixedEnv {
    fn now(&mut self) -> i64 {
        self.clock
    }

    fn rand(&mut self) -> f64 {
        if self.seq.is_empty() {
            return 0.5;
        }
        let v = self.seq[self.pos % self.seq.len()];
        self.pos += 1;
        v
    }
}
