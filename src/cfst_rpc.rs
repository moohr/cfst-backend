use crate::aliyundns::AliyunDNSClient;
use crate::cfst_rpc::cloudflare_speedtest_server::CloudflareSpeedtest;
use crate::model::NodeInfo;
use crate::{model, util};
use std::collections::HashMap;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use uuid::Uuid;
use versions::Version;

tonic::include_proto!("cfst_rpc");

#[derive(Default, Clone)]
pub struct CloudflareSpeedtestService {
    pub node_info: Arc<Mutex<HashMap<String, NodeInfo>>>,
    pub aliyun_dnsclient: AliyunDNSClient,
    pub upgrade_url: String,
    pub bootstrap_token: String,
    pub version: Version,
    pub ip_ranges: Vec<String>,
    pub minimum_mbps: i32,
    pub maximum_ping: i32,
    pub speedtest_url: String,
    pub active_streams:
        Arc<RwLock<HashMap<String, mpsc::Sender<Result<SpeedtestResponse, Status>>>>>,
}

#[tonic::async_trait]
impl CloudflareSpeedtest for Arc<CloudflareSpeedtestService> {
    /// Clients must call bootstrap to obtain session_token for authentication
    async fn bootstrap(
        &self,
        request: Request<BootstrapRequest>,
    ) -> Result<Response<BootstrapResponse>, Status> {
        let remote_addr = match request.remote_addr().unwrap().ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Ok(Response::new(BootstrapResponse {
                    success: false,
                    should_upgrade: false,
                    message: "Client connected with IPv6, which is unsupported.".to_string(),
                    session_token: "".to_string(),
                }));
            }
        };
        let req = request.into_inner();
        let token = Uuid::new_v4().to_string();
        if req.bootstrap_token != self.bootstrap_token {
            log::warn!(
                "[{}]: Invalid bootstrap attempt from node_id: {}",
                remote_addr,
                req.node_id
            );
            return Ok(Response::new(BootstrapResponse {
                success: false,
                should_upgrade: false,
                message: "Invalid bootstrap token".to_string(),
                session_token: "".to_string(),
            }));
        } else {
            let (isp, province) = util::get_client_isp_province(remote_addr).unwrap();
            log::info!(
                "[{}]: Bootstrap request from node_id: {}, ISP: {}, Province: {}",
                remote_addr,
                req.node_id,
                &isp,
                &province
            );
            self.node_info.lock().unwrap().insert(
                req.node_id.clone(),
                NodeInfo {
                    node_id: req.node_id.clone(),
                    province,
                    isp,
                    session_token: token.clone(),
                    maximum_mbps: req.maximum_mbps,
                },
            );
            Ok(Response::new(BootstrapResponse {
                success: true,
                should_upgrade: false,
                message: "Success".to_string(),
                session_token: token,
            }))
        }
    }

    type SpeedtestStream = Pin<Box<dyn Stream<Item = Result<SpeedtestResponse, Status>> + Send>>;
    /// Clients should call speedtest to register for speedtest calls, which would be streamed by the server when requested
    async fn speedtest(
        &self,
        request: Request<SpeedtestRequest>,
    ) -> Result<Response<Self::SpeedtestStream>, Status> {
        let (tx, rx) = mpsc::channel(512);
        let remote_addr = request.remote_addr().unwrap();
        let req = request.into_inner();
        let node_id = req.node_id.clone();
        let session_token = req.session_token.clone();
        match self.node_info.lock().unwrap().get(&node_id) {
            Some(node) => {
                if node.session_token != session_token {
                    log::warn!(
                        "[{}]: Unauthorized speedtest stream request from node_id {}",
                        remote_addr,
                        node_id
                    );
                    return Ok(Response::new(Box::pin(ReceiverStream::new(rx))));
                }
                log::info!(
                    "[{}]: Speedtest task stream established to node_id {}",
                    remote_addr,
                    node_id
                );
                self.active_streams
                    .write()
                    .unwrap()
                    .insert(node_id.clone(), tx);
                Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
            }
            None => {
                log::warn!(
                    "[{}]: Invalid stream establishment attempt from node_id {}",
                    remote_addr,
                    node_id
                );
                Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
            }
        }
    }

    /// Clients should upload speedtest results through this endpoint
    async fn speedtest_result(
        &self,
        request: Request<SpeedtestResultRequest>,
    ) -> Result<Response<SpeedtestResultResponse>, Status> {
        let remote_addr = match request.remote_addr().unwrap().ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Ok(Response::new(SpeedtestResultResponse {
                    success: false,
                    message: "Client connected with IPv6, which is unsupported.".to_string(),
                }));
            }
        };
        let req = request.into_inner();
        let this_node: NodeInfo = match self.node_info.lock().unwrap().get(&req.node_id) {
            Some(node) => node.clone(),
            None => {
                log::warn!(
                    "[{}]: Speedtest result upload from unknown node_id: {}",
                    remote_addr,
                    req.node_id
                );
                return Ok(Response::new(SpeedtestResultResponse {
                    success: false,
                    message: "Unauthorized node_id".to_string(),
                }));
            }
        };
        if this_node.session_token != req.session_token {
            log::warn!(
                "[{}]: Speedtest result upload from unauthorized node_id: {}",
                remote_addr,
                req.node_id
            );
            return Ok(Response::new(SpeedtestResultResponse {
                success: false,
                message: "Unauthorized session_token".to_string(),
            }));
        };
        // upload speedtest result to aliyun dns
        let isp = self
            .node_info
            .lock()
            .unwrap()
            .get(&req.node_id)
            .unwrap()
            .isp
            .clone();
        let province = self
            .node_info
            .lock()
            .unwrap()
            .get(&req.node_id)
            .unwrap()
            .province
            .clone();
        match self
            .aliyun_dnsclient
            .new_update(
                &req.ip_results.first().unwrap().ip_address,
                &isp,
                province.as_str(),
            )
            .await
        {
            Ok(_) => {
                log::info!(
                    "[{}]: Speedtest result uploaded from node_id: {}",
                    remote_addr,
                    req.node_id
                );
                Ok(Response::new(SpeedtestResultResponse {
                    success: true,
                    message: "Success".to_string(),
                }))
            }
            Err(e) => {
                log::error!(
                    "[{}]: Speedtest result upload from node_id: {} failed: {}",
                    remote_addr,
                    req.node_id,
                    e
                );
                Ok(Response::new(SpeedtestResultResponse {
                    success: false,
                    message: "Failed to upload speedtest result".to_string(),
                }))
            }
        }
    }

    /// Clients should call this endpoint to request an upgrade
    async fn upgrade(
        &self,
        request: Request<UpgradeRequest>,
    ) -> Result<Response<UpgradeResponse>, Status> {
        Ok(Response::new(UpgradeResponse {
            success: true,
            message: String::from("Success"),
            upgrade_url: self.upgrade_url.clone(),
        }))
    }

    /// Reserved for health check
    async fn alive(&self, request: Request<Ping>) -> Result<Response<Pong>, Status> {
        Ok(Response::new(Pong {}))
    }
}
