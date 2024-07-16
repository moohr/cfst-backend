use crate::qqwry;
use crate::qqwry::IpInfo;
use pinyin::ToPinyin;
use std::env::VarError;
use std::error;
use std::error::Error;
use std::net::Ipv4Addr;

/// Checks if IP is valid
/// Prevents misbehaving clients from causing a panic. Usually caught by aliyun but might as well save an API call.
pub fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<Ipv4Addr>().is_ok()
}

/// Gets the value of the environment variable `env_name` and returns it as a `String`.
/// Error handling is for weaklings. Who needs it when it can just fail successfully? It's not like people don't know how to analyze tracebacks
/// Never mind, people like that exists
///
pub fn get_env(env_name: &str) -> Result<String, VarError> {
    match std::env::var(env_name) {
        Ok(n) => Ok(n),
        Err(e) => {
            log::error!("Failed to get environment variable: {}", e);
            log::error!(
                "Check your .ENV file. Does the variable {} exist?",
                env_name
            );
            Err(e)
        }
    }
}

pub fn get_client_isp_province(remote_addr: Ipv4Addr) -> Result<(String, String), Box<dyn Error>> {
    log::debug!("Reading IP information from assets/qqwry.dat");
    let ip_info = qqwry::QQWry::new()
        .unwrap()
        .lookup(remote_addr)
        .unwrap_or_else(|| IpInfo {
            country: "Unknown".to_string(),
            area: "Unknown".to_string(),
            end_ip: Ipv4Addr::new(0, 0, 0, 0),
            start_ip: Ipv4Addr::new(0, 0, 0, 0),
        });
    // convert chinese in ip_info into english provincial names for aliyun API
    let province = ip_info.country.split("–").nth(1);
    let province_pinyin: String;
    match province {
        Some(province) => {
            // handle special cases
            if province == "陕西" {
                province_pinyin = "shaanxi".to_string();
            } else {
                province_pinyin = province
                    .to_pinyin()
                    .map(|x| x.unwrap().plain().to_string())
                    .collect::<Vec<String>>()
                    .join("");
            }
        }
        None => {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Failed to get province from IP. IP: {}, Country: {}, Area: {}",
                    remote_addr, ip_info.country, ip_info.area
                ),
            )));
        }
    }

    let isp = match ip_info.area.as_str() {
        area if area.contains("电信") => "telecom",
        area if area.contains("移动") => "mobile",
        area if area.contains("联通") => "unicom",
        _ => "telecom",
    };
    Ok((isp.parse().unwrap(), province_pinyin))
}
