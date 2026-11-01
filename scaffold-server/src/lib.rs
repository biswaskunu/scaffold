use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub mod api;
pub mod room;
pub mod sync_protocol;
mod ws_handler;

#[cfg(test)]
mod spike_loro {
    #[test]
    fn loro_round_trip() {
        super::loro_spike::test_loro_round_trip();
    }
}

#[cfg(test)]
#[path = "spike_loro.rs"]
mod loro_spike;

pub async fn run(addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let router = api::router();
    axum::serve(listener, router).await?;
    Ok(())
}
