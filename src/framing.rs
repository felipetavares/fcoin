use super::blockchain;

use futures::prelude::*;
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_serde::SymmetricallyFramed;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use serde::{Deserialize, Serialize};

pub struct Connection {
    serialized: SymmetricallyFramed<
        Framed<TcpStream, LengthDelimitedCodec>,
        Frame,
        SymmetricalBincode<Frame>,
    >,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Frame {
    Block(blockchain::Block),
    Transaction(blockchain::Transaction),
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            serialized: SymmetricallyFramed::new(
                Framed::new(stream, LengthDelimitedCodec::new()),
                SymmetricalBincode::<Frame>::default(),
            ),
        }
    }

    pub async fn read(&mut self) -> Option<Frame> {
        self.serialized.try_next().await.unwrap()
    }

    pub async fn write(&mut self, frame: Frame) {
        self.serialized.send(frame).await.unwrap();
    }
}
