//! OPC-UA boolean trigger reader — mock for CI; real client hook (Phase 3).

use std::sync::atomic::{AtomicU64, Ordering};

pub struct OpcUaConfig {
    pub endpoint: String,
    pub node_id: String,
    pub mock: bool,
    pub poll_ms: u64,
}

pub struct BoolReader {
    mock_polls: AtomicU64,
}

impl BoolReader {
    pub async fn connect(config: &OpcUaConfig) -> std::io::Result<Self> {
        if !config.mock {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!(
                    "OPC-UA client not linked for {} node {} — set SFI_OPCUA_MOCK=1 (see hal-ext/opcua-plc-trigger README)",
                    config.endpoint, config.node_id
                ),
            ));
        }
        Ok(Self {
            mock_polls: AtomicU64::new(0),
        })
    }

    pub async fn read_bool(&mut self) -> std::io::Result<bool> {
        let n = self.mock_polls.fetch_add(1, Ordering::Relaxed);
        Ok(n >= 1)
    }
}

pub fn parse_config() -> OpcUaConfig {
    let mock = std::env::var("SFI_OPCUA_MOCK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    OpcUaConfig {
        endpoint: std::env::var("SFI_OPCUA_ENDPOINT")
            .unwrap_or_else(|_| "opc.tcp://127.0.0.1:4840".into()),
        node_id: std::env::var("SFI_OPCUA_NODE").unwrap_or_else(|_| "ns=2;i=2".into()),
        mock,
        poll_ms: std::env::var("SFI_OPCUA_POLL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_bool_rises() {
        let cfg = OpcUaConfig {
            endpoint: "opc.tcp://127.0.0.1:4840".into(),
            node_id: "ns=2;i=2".into(),
            mock: true,
            poll_ms: 10,
        };
        let mut reader = BoolReader::connect(&cfg).await.unwrap();
        assert!(!reader.read_bool().await.unwrap());
        assert!(reader.read_bool().await.unwrap());
    }
}
