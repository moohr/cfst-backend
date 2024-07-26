#![allow(unused)]

mod aliyundns;
mod cfst_rpc;
mod logging;
mod model;
mod qqwry;
mod util;

use crate::aliyundns::AliyunDNSClient;
use crate::cfst_rpc::cloudflare_speedtest_server::{
    CloudflareSpeedtest, CloudflareSpeedtestServer,
};
use crate::cfst_rpc::{
    BootstrapRequest, BootstrapResponse, CloudflareSpeedtestService, IpResult, Ping, Pong,
    SpeedtestRequest, SpeedtestResponse, SpeedtestResultRequest, SpeedtestResultResponse,
    UpgradeRequest, UpgradeResponse,
};
use crate::model::NodeInfo;
use crate::util::get_env;
use dotenv::dotenv;
use log::LevelFilter;
use pinyin::ToPinyin;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc;
use tokio::time::{self, interval, Duration};
use tokio_stream::Stream;
use tonic::codegen::tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use uuid::Uuid;
use versions::Version;

#[macro_use]
extern crate log;
extern crate simple_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    logging::init();
    log::info!("Initializing backend");
    let listen_addr = format!("0.0.0.0:{}", get_env("LISTEN_PORT").unwrap()).parse()?;
    let cfst_backend = Arc::new(CloudflareSpeedtestService {
        node_info: Arc::new(Mutex::new(HashMap::new())),
        aliyun_dnsclient: AliyunDNSClient {
            api_access_key: get_env("ALIYUN_ACCESS_KEY").unwrap(),
            dns_root_domain: get_env("DNS_ROOT_DOMAIN").unwrap(),
            dns_record_name: get_env("DNS_SUB_DOMAIN").unwrap(),
            api_access_secret: get_env("ALIYUN_ACCESS_SECRET").unwrap(),
        },
        upgrade_url: format!("https://ghp.rtc.ovh/https://github.com/GenshinMinecraft/CloudflareSpeedtest-Slave/releases/download/v{}/CloudflareSpeedtest-Slave", env!("CARGO_PKG_VERSION")),
        bootstrap_token: get_env("BOOTSTRAP_TOKEN").unwrap(),
        version: Version::parse(env!("CARGO_PKG_VERSION")).unwrap().1,
        active_streams: Arc::new(RwLock::new(HashMap::new())),
        ip_ranges: std::env::var("IP_RANGES")
            .unwrap()
            .to_string()
            .split("\\")
            .map(|x| x.to_string())
            .collect(),
        minimum_mbps: get_env("MINIMUM_MBPS").unwrap().parse::<i32>().unwrap(),
        maximum_ping: get_env("MAXIMUM_PING").unwrap().parse::<i32>().unwrap(),
        speedtest_url: get_env("SPEEDTEST_LINK").unwrap(),
    });
    let cfst_backend_clone = Arc::clone(&cfst_backend);
    let test_interval = get_env("TEST_INTERVAL").unwrap().parse::<u64>().unwrap();
    tokio::spawn(async move {
        log::info!(
            "Speedtest cron started with cycle of {} seconds",
            test_interval
        );
        let mut interval_timer = interval(Duration::from_secs(test_interval));

        loop {
            interval_timer.tick().await;
            let senders = {
                let active_streams = cfst_backend_clone.active_streams.read().unwrap();
                active_streams
                    .iter()
                    .map(|(node_id, sender)| (node_id.clone(), sender.clone()))
                    .collect::<Vec<_>>()
            };

            for (node_id, tx) in senders {
                let client_reported_maximum_mbps = {
                    let node_info = cfst_backend_clone.node_info.lock().unwrap();
                    node_info.get(&node_id).unwrap().maximum_mbps
                };
                let speedtest_response = SpeedtestResponse {
                    ip_ranges: cfst_backend_clone.ip_ranges.clone(),
                    minimum_mbps: std::cmp::min(
                        cfst_backend_clone.minimum_mbps,
                        client_reported_maximum_mbps,
                    ),
                    maximum_ping: cfst_backend_clone.maximum_ping,
                    speed_url: cfst_backend_clone.speedtest_url.clone(),
                };
                match tx.send(Ok(speedtest_response)).await {
                    Ok(_) => {
                        log::info!("Sent speedtest call to client {}", node_id);
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to send speedtest response to client {}: {}",
                            node_id,
                            e
                        );
                        // Remove dead client
                        let mut active_streams = cfst_backend_clone.active_streams.write().unwrap();
                        active_streams.remove(&node_id);
                        log::info!("Removed disconnected client {}", node_id);
                    }
                };
            }
        }
    });

    log::info!(
        "Starting gRPC server on 0.0.0.0:{}",
        get_env("LISTEN_PORT").unwrap()
    );
    Server::builder()
        .add_service(CloudflareSpeedtestServer::new(cfst_backend))
        .serve(listen_addr)
        .await?;
    Ok(())
}
