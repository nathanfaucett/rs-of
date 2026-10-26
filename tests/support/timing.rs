struct SetupTimingLog;

impl log::Log for SetupTimingLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
            || metadata.level() == log::Level::Info
                && (metadata.target() == "unified_server::local_provisioning"
                    || metadata.target() == "idp_server::hosted_setup")
            || metadata.level() == log::Level::Debug
                && [
                    "idp_server::independent",
                    "device_server::independent",
                    "storage_server::independent",
                    "storage_server::idp_client",
                    "device_server::router::routes::replication",
                ]
                .iter()
                .any(|target| metadata.target().starts_with(*target))
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            if std::env::var_os("OF_SETUP_TIMEOUT_TIMING").is_some() {
                eprintln!(
                    "pid={} wall_ms={} target={} {}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system clock")
                        .as_millis(),
                    record.target(),
                    record.args()
                );
            } else {
                eprintln!("{}", record.args());
            }
        }
    }

    fn flush(&self) {}
}

static SETUP_TIMING_LOG: SetupTimingLog = SetupTimingLog;

pub(crate) fn capture_setup_timings() {
    log::set_logger(&SETUP_TIMING_LOG).expect("isolated timing logger");
    log::set_max_level(log::LevelFilter::Debug);
}
