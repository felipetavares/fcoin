#[macro_use]
extern crate serde_big_array;
big_array! { BigArray; }

mod blockchain;
mod framing;

use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use framing::{Connection, Frame};

#[derive(Deserialize)]
struct Configuration {
    port: u16,
    seeds: Vec<SocketAddr>,
}

const CONFIGURATION_FILE_PATH: &str = "fcoin.toml";

#[tokio::main]
async fn main() {
    println!("Starting fcoin server...");

    match std::fs::read_to_string(CONFIGURATION_FILE_PATH) {
        Ok(content) => match toml::from_str(&content) {
            Ok(configuration) => accept_connections_loop(configuration).await,
            Err(issue) => panic!(
                "Could not parse {}. Error: {}",
                CONFIGURATION_FILE_PATH, issue
            ),
        },
        Err(issue) => panic!(
            "Could not find the {} configuration file. Error: {}",
            CONFIGURATION_FILE_PATH, issue
        ),
    }
}

// Binds to the port in the configuration file and spawns a `peer_loop` for each
// of the connections created.
async fn accept_connections_loop(conf: Configuration) {
    println!("{:?}", conf.seeds);

    let listener = TcpListener::bind(format!("localhost:{}", conf.port))
        .await
        .unwrap();

    let (tx, rx) = mpsc::channel(1);
    let node = Arc::new(Mutex::new(blockchain::Node::new()));

    {
        let node_clone = node.clone();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            blockchain::block_generator(node_clone, rx, tx_clone).await;
        });
    }

    for seed in conf.seeds {
        match TcpStream::connect(seed).await {
            Ok(stream) => {
                let node_clone = node.clone();
                let tx_clone = tx.clone();

                tokio::spawn(async move {
                    peer_loop(node_clone, tx_clone, stream, seed).await;
                });
            }
            Err(issue) => println!(
                "Could not connect to the {} hardcoded seed node: {}",
                seed, issue
            ),
        }
    }

    loop {
        let (stream, address) = listener.accept().await.unwrap();
        let node_clone = node.clone();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            peer_loop(node_clone, tx_clone, stream, address).await;
        });
    }
}

// await is spanwed for each connected peer
async fn peer_loop(
    node: Arc<Mutex<blockchain::Node>>,
    tx: mpsc::Sender<blockchain::ProtoBlock>,
    stream: TcpStream,
    address: SocketAddr,
) {
    println!("Connected with {}.", address);

    let (writter, mut reader) = Connection::new(stream);

    node.lock().await.add_peer(address, writter);

    loop {
        match reader.read().await {
            Some(Frame::Block(block)) => blockchain::block_received(node.clone(), block).await,
            Some(Frame::Transaction(trx)) => {
                blockchain::transaction_received(trx, tx.clone()).await
            }
            None => break,
        }
    }
}
