use log::LevelFilter;

pub fn init() {
    match std::env::var("LOG_LEVEL").unwrap().as_str() {
        "DEBUG" => {
            simple_logger::SimpleLogger::new()
                .with_level(LevelFilter::Debug)
                .env()
                .init()
                .unwrap();
            log::info!("Initializing backend, log level: DEBUG");
        }
        "INFO" => {
            simple_logger::SimpleLogger::new()
                .with_level(LevelFilter::Info)
                .env()
                .init()
                .unwrap();
            log::info!("Initializing backend, log level: INFO");
        }
        "WARN" => {
            simple_logger::SimpleLogger::new()
                .with_level(LevelFilter::Warn)
                .env()
                .init()
                .unwrap();
            log::info!("Initializing backend, log level: WARN");
        }

        _ => {
            panic!("Invalid log level set, please choose from INFO, WARN or DEBUG");
        }
    }
}
