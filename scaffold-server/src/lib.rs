use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub mod api;
pub mod room;
pub mod store;
pub mod sync_protocol;
mod ws_handler;

#[cfg(test)]
mod spike_loro;

pub async fn run(addr: SocketAddr) -> Result<()> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://scaffold.db".to_string());
    let store = store::Store::new(&database_url).await.ok();
    let rooms = room::RoomManager::new(store);

    let listener = TcpListener::bind(addr).await?;
    let router = api::router_with_rooms(rooms);
    axum::serve(listener, router).await?;
    Ok(())
}
