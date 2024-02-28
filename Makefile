run:
	RUST_LOG="trace,klickhouse=off,reqwest=off,hyper=off,h2=off,datafusion=off,sqlparser=off,tokio_util=off,rustls=off,tracing=off" cargo run --release
