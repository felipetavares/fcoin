use super::framing;
use crate::BigArray;
use num::BigUint;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use tokio::sync::mpsc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    #[serde(with = "BigArray")]
    source_public_key: PublicKey,
    #[serde(with = "BigArray")]
    destination_public_key: PublicKey,
    amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    details: TransactionDetails,
    #[serde(with = "BigArray")]
    source_signature: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    time: u64,
    // This is used to give whoever created this block a +1 balance
    #[serde(with = "BigArray")]
    node_public_key: PublicKey,
    // Linking to the previous block
    previous_hash: Hash,
    // Used for the proof-of-work
    // (increment this until the hash of the block is < n)
    nonce: [u8; 32],
    // The actual money transfer in this block
    transaction: Transaction,
}

pub struct ProtoBlock {
    nonce: [u8; 32],
    transaction: Transaction,
}

type Hash = [u8; 32];
type Signature = [u8; 128];
type PublicKey = [u8; 128];
type Blockchain = HashMap<Hash, Block>;

struct HashFmt(Hash);
struct PublicKeyFmt(PublicKey);
struct BlockchainFmt(Blockchain, Hash);

pub struct Node {
    public_key: [u8; 128],
    blockchain: Blockchain,
    tip_hash: Hash,
    peers: HashMap<SocketAddr, framing::WriteConnection>,
}

impl Node {
    pub fn new() -> Node {
        Node {
            public_key: read_public_key_from_disk(),
            blockchain: HashMap::new(),
            tip_hash: [0; 32],
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, addr: SocketAddr, con: framing::WriteConnection) {
        self.peers.insert(addr, con);
    }
}

impl TransactionDetails {
    pub fn new(source: PublicKey, destination: PublicKey, amount: u64) -> TransactionDetails {
        TransactionDetails {
            source_public_key: source,
            destination_public_key: destination,
            amount: amount,
        }
    }
}

impl Transaction {
    pub fn new(details: TransactionDetails, signature: [u8; 128]) -> Transaction {
        Transaction {
            details: details,
            source_signature: signature,
        }
    }
}

impl std::fmt::Display for BlockchainFmt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut tip = self.1;

        loop {
            match self.0.get(&tip) {
                Some(block) => {
                    write!(f, "{}\n", block.transaction)?;
                    tip = block.previous_hash;
                }
                None => return Ok(()),
            }
        }
    }
}

impl std::fmt::Display for HashFmt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{:02x}", byte)?
        }

        Ok(())
    }
}

impl std::fmt::Display for PublicKeyFmt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in &self.0[0..8] {
            write!(f, "{:02x}", byte)?
        }

        Ok(())
    }
}

impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "transfer ${} from {} to {}",
            self.details.amount,
            PublicKeyFmt(self.details.source_public_key),
            PublicKeyFmt(self.details.destination_public_key),
        )
    }
}

pub fn sign(details: &TransactionDetails) -> Signature {
    [0; 128]
}

fn read_public_key_from_disk() -> PublicKey {
    [0; 128]
}

fn transaction_to_proto_block(transaction: Transaction) -> ProtoBlock {
    ProtoBlock {
        nonce: [0; 32],
        transaction: transaction,
    }
}

fn to_32bytes(byte_vector: &[u8]) -> [u8; 32] {
    let n = byte_vector.len();

    let mut bytes: [u8; 32] = [0; 32];
    let slice = &mut bytes[32 - n..32];

    slice.copy_from_slice(byte_vector);

    bytes
}

fn hash_block(block: &Block) -> Hash {
    let hasher = Sha256::new();

    to_32bytes(
        &hasher
            .chain(block.time.to_le_bytes())
            .chain(&block.node_public_key)
            .chain(&block.previous_hash)
            .chain(&block.nonce)
            .chain(&block.transaction.source_signature)
            .chain(&block.transaction.details.source_public_key)
            .chain(&block.transaction.details.destination_public_key)
            .chain(block.transaction.details.amount.to_le_bytes())
            .finalize(),
    )
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn amount(
    mut value: i128,
    blockchain: &Blockchain,
    tip_hash: &Hash,
    id: &PublicKey,
) -> Result<i128, String> {
    if tip_hash == &[0; 32] {
        Ok(value)
    } else {
        match blockchain.get(tip_hash) {
            Some(block) => {
                // TODO: cannot process transactions that involve ourselves only

                if block.transaction.details.source_public_key
                    == block.transaction.details.destination_public_key
                {
                    return Err("Source and destination are the same!".to_string());
                }

                if id == &block.transaction.details.source_public_key {
                    value -= block.transaction.details.amount as i128;
                }

                if id == &block.transaction.details.destination_public_key {
                    value += block.transaction.details.amount as i128;
                }

                if id == &block.node_public_key {
                    value += 1;
                }

                amount(value, blockchain, &block.previous_hash, id)
            }
            None => Err("Previous hash not found in the blockchain!".to_string()),
        }
    }
}

// TODO: verifying signatures
fn valid_block(block: &Block, blockchain: &Blockchain) -> bool {
    match amount(
        0,
        blockchain,
        &block.previous_hash,
        &block.transaction.details.source_public_key,
    ) {
        Ok(value) => {
            println!(
                "FUNDS CHECK: {} has ${}. Trying to transfer ${}",
                PublicKeyFmt(block.transaction.details.source_public_key),
                value,
                block.transaction.details.amount
            );

            value >= block.transaction.details.amount as i128
                && block.transaction.details.source_public_key
                    != block.transaction.details.destination_public_key
        }
        Err(err) => {
            println!("{}", err);

            false
        }
    }
}

pub async fn block_received(node: Arc<Mutex<Node>>, block: Block) {
    let hash = hash_block(&block);
    let mut node = node.lock().await;

    println!("BLOCKCHAIN TIP IS {}", HashFmt(node.tip_hash));
    println!("BLOCK HASH IS {}", HashFmt(hash));
    println!("BLOCK PREVIOUS HASH IS {}", HashFmt(block.previous_hash));

    match node.blockchain.get(&hash) {
        Some(_) => println!("BLOCKCHAIN ALREADY HAS BLOCK. STOPPING."),
        None => {
            if valid_block(&block, &node.blockchain) {
                println!("BLOCK IS VALID");

                // FIXME: handling timestamps
                if block.previous_hash == node.tip_hash {
                    node.tip_hash = hash;
                }

                node.blockchain.insert(hash, block);

                println!("** BLOCK ADDED TO BLOCKCHAIN **");
                println!("{}", BlockchainFmt(node.blockchain.clone(), node.tip_hash));
            }
        }
    }
}

async fn block_created(node: Arc<Mutex<Node>>, block: Block) {
    block_received(node, block).await
}

pub async fn transaction_received(transaction: Transaction, tx: mpsc::Sender<ProtoBlock>) {
    println!("TRANSACTION {}", transaction);

    match tx.send(transaction_to_proto_block(transaction)).await {
        Ok(_) => (),
        Err(_) => (),
    }

    // TODO: replicate transaction in the network
}

async fn proof_of_work(
    node: Arc<Mutex<Node>>,
    proto_block: ProtoBlock,
) -> Result<Block, ProtoBlock> {
    let unlocked_node = node.lock().await;

    let block = Block {
        time: timestamp(),
        node_public_key: unlocked_node.public_key,
        previous_hash: unlocked_node.tip_hash,
        nonce: proto_block.nonce,
        transaction: proto_block.transaction.clone(),
    };

    let hash = hash_block(&block);

    println!("PROOF OF WORK {}", HashFmt(hash));

    if BigUint::from_bytes_le(&hash) < BigUint::from(2u32).pow(255) - BigUint::from(1u32) {
        println!("PROOF OF WORK ACCEPTED");

        Ok(block)
    } else {
        println!("PROOF OF WORK WRONG");

        Err(ProtoBlock {
            nonce: to_32bytes(
                &(BigUint::from_bytes_le(&proto_block.nonce) + BigUint::from(1u32)).to_bytes_le(),
            ),
            transaction: proto_block.transaction,
        })
    }
}

pub async fn block_generator(
    node: Arc<Mutex<Node>>,
    mut rx: mpsc::Receiver<ProtoBlock>,
    tx: mpsc::Sender<ProtoBlock>,
) {
    loop {
        match rx.recv().await {
            Some(proto_block) => match proof_of_work(node.clone(), proto_block).await {
                Ok(block) => block_created(node.clone(), block).await,
                Err(proto_block) => match tx.send(proto_block).await {
                    Ok(()) => {}
                    Err(_) => {}
                },
            },
            None => {}
        }
    }
}
