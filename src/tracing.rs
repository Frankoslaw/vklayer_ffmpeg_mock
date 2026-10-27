use std::sync::Once;

static LOGGER: Once = Once::new();

pub fn init() {
    LOGGER.call_once(|| {
        let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
            .try_init();
    });
}

/// Wrap one downstream call with entry, result, and elapsed-time logging.
#[macro_export]
macro_rules! trace_call {
    ($function:expr, $summary:expr, $call:expr) => {{
        $crate::tracing::init();
        let started = std::time::Instant::now();
        ::log::debug!("{}({})", $function, $summary);
        let result = $call;
        ::log::info!("{} -> {:?} ({:?})", $function, result, started.elapsed());
        result
    }};
}
