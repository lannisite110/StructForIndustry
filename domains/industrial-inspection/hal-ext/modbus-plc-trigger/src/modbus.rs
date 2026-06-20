//! Modbus TCP coil reader — rising edge triggers HAL frame (like `sfi-plc-trigger`).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio_modbus::client::tcp;
use tokio_modbus::client::Context;
use tokio_modbus::prelude::*;

pub struct ModbusConfig {
    pub addr: SocketAddr,
    pub coil: u16,
    pub mock: bool,
    pub poll_ms: u64,
}

pub struct CoilReader {
    mock: bool,
    mock_polls: AtomicU64,
    ctx: Option<Context>,
    coil: u16,
}

impl CoilReader {
    pub async fn connect(config: &ModbusConfig) -> std::io::Result<Self> {
        if config.mock {
            return Ok(Self {
                mock: true,
                mock_polls: AtomicU64::new(0),
                ctx: None,
                coil: config.coil,
            });
        }
        let ctx = tcp::connect(config.addr)
            .await
            .map_err(|e| std::io::Error::other(format!("modbus connect {}: {e}", config.addr)))?;
        Ok(Self {
            mock: false,
            mock_polls: AtomicU64::new(0),
            ctx: Some(ctx),
            coil: config.coil,
        })
    }

    pub async fn read_coil(&mut self) -> std::io::Result<bool> {
        if self.mock {
            let n = self.mock_polls.fetch_add(1, Ordering::Relaxed);
            // Poll 0: low, poll 1+: high (single rising edge for E2E)
            return Ok(n >= 1);
        }
        let ctx = self
            .ctx
            .as_mut()
            .ok_or_else(|| std::io::Error::other("modbus context missing"))?;
        let coils = ctx
            .read_coils(self.coil, 1)
            .await
            .map_err(|e| std::io::Error::other(format!("read_coils transport: {e}")))?
            .map_err(|e| std::io::Error::other(format!("read_coils modbus: {e}")))?;
        Ok(coils.first().copied().unwrap_or(false))
    }
}

pub fn parse_config() -> ModbusConfig {
    let mock = std::env::var("SFI_MODBUS_MOCK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let addr: SocketAddr = std::env::var("SFI_MODBUS_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:502".into())
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:502".parse().unwrap());
    let coil: u16 = std::env::var("SFI_MODBUS_COIL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let poll_ms: u64 = std::env::var("SFI_MODBUS_POLL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    ModbusConfig {
        addr,
        coil,
        mock,
        poll_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_coil_rises_after_first_poll() {
        let cfg = ModbusConfig {
            addr: "127.0.0.1:502".parse().unwrap(),
            coil: 0,
            mock: true,
            poll_ms: 10,
        };
        let mut reader = CoilReader::connect(&cfg).await.unwrap();
        assert!(!reader.read_coil().await.unwrap());
        assert!(reader.read_coil().await.unwrap());
    }
}
