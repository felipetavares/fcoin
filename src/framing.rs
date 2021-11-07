use super::blockchain;

use futures::prelude::*;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_serde::SymmetricallyFramed;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use serde::{Deserialize, Serialize};

pub struct Connection {}

pub struct WriteConnection {
    writter: SymmetricallyFramed<
        FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
        Frame,
        SymmetricalBincode<Frame>,
    >,
}

pub struct ReadConnection {
    reader: SymmetricallyFramed<
        FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
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
    pub fn new(stream: TcpStream) -> (WriteConnection, ReadConnection) {
        let (rx, tx) = stream.into_split();

        (
            WriteConnection {
                writter: SymmetricallyFramed::new(
                    FramedWrite::new(tx, LengthDelimitedCodec::new()),
                    SymmetricalBincode::<Frame>::default(),
                ),
            },
            ReadConnection {
                reader: SymmetricallyFramed::new(
                    FramedRead::new(rx, LengthDelimitedCodec::new()),
                    SymmetricalBincode::<Frame>::default(),
                ),
            },
        )
    }
}

impl ReadConnection {
    pub async fn read(&mut self) -> Option<Frame> {
        self.reader.try_next().await.unwrap()
    }
}

impl WriteConnection {
    pub async fn write(&mut self, frame: Frame) {
        self.writter.send(frame).await.unwrap();
    }
}
