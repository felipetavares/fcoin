#[macro_use]
extern crate serde_big_array;
big_array! { BigArray; }

mod blockchain;
mod framing;

use framing::{Connection, Frame};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    println!("Sending fcoin...");

    let stream = TcpStream::connect("localhost:7123").await.unwrap();
    let (mut writter, _) = Connection::new(stream);

    // TODO: Fetch this information from command line
    let details = blockchain::TransactionDetails::new([1; 128], [2; 128], 5);
    let signature = blockchain::sign(&details);

    writter
        .write(Frame::Transaction(blockchain::Transaction::new(
            details, signature,
        )))
        .await;
}
