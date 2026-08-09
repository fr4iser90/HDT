//! Battlegrounds Companion core library.

pub mod cards;
pub mod combat;
pub mod discover;
pub mod entities;
pub mod log_config;
pub mod parser;
pub mod runtime;
pub mod state;
pub mod watch;

pub use cards::{
    card_db_count, card_db_loaded, card_def, card_name, ensure_card_data_loaded, sync_card_db,
    CardDef, CardDb,
};
pub use combat::{CombatOdds, FightMode};
pub use discover::{
    discover_hs_paths, newest_log_session, newest_power_log, DiscoverReport, HsPaths,
};
pub use log_config::{ensure_log_config, DEFAULT_LOG_CONFIG};
pub use parser::{LogEvent, PowerParser};
pub use runtime::{hearthstone_running, runtime_status, RuntimeStatus};
pub use state::{GameMode, LiveState, MatchPhase, OpponentSnapshot, QueueStatus};
pub use watch::{watch_power_log, WatchOptions, WatchSink};
