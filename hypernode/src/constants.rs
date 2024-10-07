pub const RESERVATION_DURATION_HOURS: u64 = 8;
pub const HEADER_LOOKBACK_LIMIT: usize = 200;
pub const CONFIRMATION_HEIGHT_DELTA: u64 = 1;
pub const CHALLENGE_PERIOD_MINUTES: u64 = 10;
pub const MAIN_ELF: &[u8] = include_bytes!("../../circuits/elf/riscv32im-succinct-zkvm-elf");
