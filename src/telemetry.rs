use tracing_subscriber::layer::SubscriberExt;
/// # Panics
///
/// if the global default subscriber cannot be set
/// and if the connection to the database fails
///
#[must_use]
pub fn get_subscriber(debug: bool) -> impl tracing::Subscriber + Send + Sync {
    let env_filter = if debug {
        "trace".to_string()
    } else {
        "info".to_string()
    };
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(env_filter));
    let env_filter =
        env_filter.add_directive("actix_http=info".parse().expect("Invalid directive"));
    let env_filter = env_filter.add_directive("hyper=info".parse().expect("Invalid directive"));
    let stdout_layer = tracing_subscriber::fmt::layer().pretty();
    let subscriber = tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(stdout_layer);
    let json_log = if debug {
        let json_log = tracing_subscriber::fmt::layer().json();
        Some(json_log)
    } else {
        None
    };
    subscriber.with(json_log)
}
/// # Panics
///
/// if the global default subscriber cannot be set
/// and if the connection to the database fails
pub fn init_subscriber(subscriber: impl tracing::Subscriber + Send + Sync) {
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
}
