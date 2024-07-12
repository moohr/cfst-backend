use crate::cfst_rpc;
use tokio::sync::mpsc::Receiver;

#[derive(Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub province: String,
    pub isp: String,
    pub session_token: String,
    pub maximum_mbps: i32,
}
