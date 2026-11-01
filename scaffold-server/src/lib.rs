use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;

mod api;
mod spike_loro;

pub async fn run(addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let router = api::router();
    axum::serve(listener, router).await?;
    Ok(())
}
