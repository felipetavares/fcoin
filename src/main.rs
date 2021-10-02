#[macro_use]
extern crate serde_big_array;
big_array! { BigArray; }

mod blockchain;
mod framing;

use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

use framing::{Connection, Frame};

#[tokio::main]
async fn main() {
    println!("Starting fcoin server...");

    let listener = TcpListener::bind("localhost:7123").await.unwrap();

    loop {
        let (socket, address) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            cryptocurrency_loop(socket, address).await;
        });
    }
}

// This is spanwed for each connected peer
async fn cryptocurrency_loop(stream: TcpStream, _address: SocketAddr) {
    let mut connection = Connection::new(stream);

    loop {
        match connection.read().await {
            Some(Frame::Block(block)) => blockchain::block_received(block).await,
            Some(Frame::Transaction(trx)) => blockchain::transaction_received(trx).await,
            None => continue,
        }
    }
}
