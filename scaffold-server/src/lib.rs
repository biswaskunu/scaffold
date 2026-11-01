use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub mod api;
pub mod room;
pub mod sync_protocol;
mod ws_handler;

#[cfg(test)]
mod spike_loro;

pub async fn run(addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let router = api::router();
    axum::serve(listener, router).await?;
    Ok(())
}
