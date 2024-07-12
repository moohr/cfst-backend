use chrono::format::strftime::StrftimeItems;
use chrono::Utc;
use hex;
use hmac::digest::typenum::Prod;
use hmac::{Hmac, Mac};
use reqwest::header::HeaderMap;
use reqwest::{Client, Request as reqwestRequest, RequestBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::error::Error;
use tokio::net::TcpStream;
use tokio::task;
use tonic::codegen::Body;
use tonic::Request;
use url::Url;
use versions::Op;

#[derive(Clone, Default)]
pub struct AliyunDNSClient {
    pub(crate) api_access_key: String,
    pub(crate) api_access_secret: String,
    pub(crate) dns_root_domain: String,
    pub(crate) dns_record_name: String,
}

#[derive(sqlx::FromRow)]
struct Record {
    record_id: Option<String>,
    isp: Option<String>,
    province: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct AddRecordResponse {
    RequestId: String,
    RecordId: String,
}

impl AliyunDNSClient {
    fn sha256(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn hmac_sha256(&self, data: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_access_secret.as_bytes()).unwrap();
        mac.update(data.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        log::debug!("HMAC-SHA256 signature: {}", signature);
        signature
    }
    fn sign_request(&self, headers: &HeaderMap, query: &str) -> String {
        let canonical_request = format!(
            "POST\n/\n{}\n{}\n\n{}\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            query,
            headers
                .iter()
                .map(|(k, v)| format!("{}:{}", k.to_string(), v.to_str().unwrap()))
                .collect::<Vec<String>>()
                .join("\n"),
            headers
                .iter()
                .map(|(k, _)| k.to_string())
                .collect::<Vec<String>>()
                .join(";")
        );
        log::debug!("[ALIYUN] Canonical request: \n{}", canonical_request);
        let string_to_sign = format!("ACS3-HMAC-SHA256\n{}", Self::sha256(&canonical_request));
        log::debug!("[ALIYUN] String to sign: {}", string_to_sign);
        self.hmac_sha256(&string_to_sign)
    }

    fn get_headers(&self, query: &str, action: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("host", "alidns.cn-hongkong.aliyuncs.com".parse().unwrap());
        headers.insert("x-acs-action", action.parse().unwrap());
        headers.insert(
            "x-acs-content-sha256",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-acs-date",
            Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert("x-acs-version", "2015-01-09".parse().unwrap());
        let signature = self.sign_request(&headers, &query);
        headers.insert(
            "authorization",
            format!(
                "ACS3-HMAC-SHA256 Credential={},SignedHeaders={},Signature={}",
                self.api_access_key,
                headers
                    .iter()
                    .map(|(k, _)| k.to_string())
                    .collect::<Vec<String>>()
                    .join(";"),
                signature
            )
            .parse()
            .unwrap(),
        );
        headers
    }

    pub async fn new_update(
        &self,
        ip: &str,
        isp: &str,
        province: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut pool = SqlitePool::connect("sqlite://aliyundns.db").await?;
        let record_id_result: Vec<Record> = sqlx::query_as!(
            Record,
            r#"SELECT * FROM records WHERE isp = ? AND province = ?"#,
            isp,
            province
        )
        .fetch_all(&pool)
        .await?;
        if record_id_result.is_empty() {
            log::info!(
                "[ALIYUN] No record for ISP: {}, Province: {} exists, so adding a new one",
                isp,
                province
            );
            let record_id = self.add_record(isp, province, ip).await?;
            log::info!("[ALIYUN] Obtained record ID: {}", record_id);
            sqlx::query!(
                r#"INSERT INTO records (record_id, isp, province) VALUES (?,?,?) "#,
                record_id,
                isp,
                province
            )
            .execute(&pool)
            .await?;
            log::info!(
                "[ALIYUN] Updated record ID for ISP: {}, Province: {}",
                isp,
                province
            );
            Ok(())
        } else {
            log::info!(
                "[ALIYUN] Record for ISP: {}, Province: {} exists, so updating it",
                isp,
                province
            );
            let record_id = record_id_result.first().unwrap().record_id.clone().unwrap();
            self.update_record(isp, province, ip, &record_id).await?;
            log::info!(
                "[ALIYUN] Updated record for ISP: {}, Province: {}",
                isp,
                province
            );
            Ok(())
        }
    }

    pub async fn update_record(
        &self,
        isp: &str,
        province: &str,
        ip: &str,
        record_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let base_url = "https://alidns.cn-hongkong.aliyuncs.com";
        let mut query = format!(
            "DomainName={}&Line={}&RR={}&RecordId={}&TTL=600&Type=A&Value={}",
            self.dns_root_domain,
            format!("cn_{}_{}", isp, province),
            self.dns_record_name,
            record_id,
            ip,
        );
        let mut headers = self.get_headers(&query, "UpdateDomainRecord");
        let client = Client::new();
        let mut request = client
            .post(&format!("{}/?{}", base_url, query))
            .headers(headers);
        log::debug!("[ALIYUN] Request: {:?}", request);
        let response = request.send().await?;
        match response.status().as_u16() {
            200 => Ok(()),
            _ => {
                log::debug!("Failed to update record: {:?}", response);
                log::debug!("Response: {}", response.text().await.unwrap());
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to update record",
                )))
            }
        }
    }

    #[allow(non_snake_case)]
    pub async fn add_record(
        &self,
        isp: &str,
        province: &str,
        ip: &str,
    ) -> Result<String, Box<dyn Error>> {
        let base_url = "https://alidns.cn-hongkong.aliyuncs.com";
        let mut query = format!(
            "DomainName={}&Line={}&RR={}&TTL=600&Type=A&Value={}",
            self.dns_root_domain,
            format!("cn_{}_{}", isp, province),
            self.dns_record_name,
            ip,
        );
        let mut headers = self.get_headers(&query, "AddDomainRecord");
        let client = Client::new();
        let mut request = client
            .post(&format!("{}/?{}", base_url, query))
            .headers(headers);
        log::debug!("[ALIYUN] Request: {:?}", request);
        let response = request.send().await?;
        match response.status().as_u16() {
            200 => {
                let aRR: AddRecordResponse = serde_json::from_str(&response.text().await.unwrap())?;
                Ok(aRR.RecordId)
            }
            _ => {
                log::debug!("Failed to add record: {:?}", response);
                log::debug!("Response: {}", response.text().await.unwrap());
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to add record",
                )))
            }
        }
    }
}
