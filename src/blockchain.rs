use crate::BigArray;
use ctrlc;
use futures::executor::block_on;
use num::BigUint;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::mpsc::channel;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(with = "BigArray")]
    source_signature: [u8; 128],
    #[serde(with = "BigArray")]
    source_public_key: [u8; 128],
    #[serde(with = "BigArray")]
    destination_public_key: [u8; 128],
    amount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    // This is used to give whoever created this block a +1 balance
    #[serde(with = "BigArray")]
    node_public_key: [u8; 128],
    // Linking to the previous block
    previous_hash: [u8; 32],
    // Used for the proof-of-work
    // (increment this until the hash of the block is < n)
    nonce: [u8; 32],
    // The actual money transfer in this block
    transaction: Transaction,
}

struct ProtoBlock {
    nonce: [u8; 32],
    transaction: Transaction,
}

type Blockchain = HashMap<[u8; 32], Block>;

fn read_public_key_from_disk() -> [u8; 128] {
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
    let slice = &mut bytes[32 - n..n - 1];

    slice.copy_from_slice(byte_vector);

    bytes
}

fn hash_block(block: &Block) -> [u8; 32] {
    let hasher = Sha256::new();

    to_32bytes(
        &hasher
            .chain(&block.node_public_key)
            .chain(&block.previous_hash)
            .chain(&block.nonce)
            .chain(&block.transaction.source_signature)
            .chain(&block.transaction.source_public_key)
            .chain(&block.transaction.destination_public_key)
            .chain(block.transaction.amount.to_le_bytes())
            .finalize(),
    )
}

fn amount(
    mut value: i128,
    blockchain: &Blockchain,
    tip_hash: &[u8; 32],
    id: &[u8; 128],
) -> Option<i128> {
    if tip_hash == &[0; 32] {
        Some(0)
    } else {
        match blockchain.get(tip_hash) {
            Some(block) => {
                if block.transaction.source_public_key == block.transaction.destination_public_key {
                    return None;
                }

                if id == &block.transaction.source_public_key {
                    value -= block.transaction.amount as i128;
                }

                if id == &block.transaction.destination_public_key {
                    value += block.transaction.amount as i128;
                }

                if id == &block.node_public_key {
                    value += 1;
                }

                if value >= 0 {
                    amount(value, blockchain, &block.previous_hash, id)
                } else {
                    None
                }
            }
            None => None,
        }
    }
}

fn valid_block(block: &Block, blockchain: &Blockchain) -> bool {
    match amount(
        0,
        blockchain,
        &block.previous_hash,
        &block.transaction.source_public_key,
    ) {
        Some(value) => value >= block.transaction.amount as i128,
        None => false,
    }
}

async fn _block_received(block: Block, blockchain: &mut Blockchain, tip_hash: &mut [u8; 32]) {
    let hash = hash_block(&block);

    match blockchain.get(&hash) {
        Some(_) => {}
        None => {
            if valid_block(&block, blockchain) {
                // FIXME: handling timestamps
                if &block.previous_hash == tip_hash {
                    *tip_hash = hash;
                }

                blockchain.insert(hash, block);
            }
        }
    }
}

pub async fn block_received(block: Block) {}

async fn block_created(block: Block) {}

async fn _transaction_received(transaction: Transaction, pending: &mut VecDeque<ProtoBlock>) {
    pending.push_back(transaction_to_proto_block(transaction));

    // TODO: replicate transaction in the network
}

pub async fn transaction_received(transaction: Transaction) {}

async fn proof_of_work(
    proto_block: ProtoBlock,
    tip_hash: &mut [u8; 32],
    public_key: &[u8; 128],
) -> Result<Block, ProtoBlock> {
    let hash = hash_block(&Block {
        node_public_key: *public_key,
        previous_hash: *tip_hash,
        nonce: proto_block.nonce,
        transaction: proto_block.transaction.clone(),
    });

    if BigUint::from_bytes_le(&hash) < BigUint::from(2u32).pow(128) {
        let previous_hash = *tip_hash;
        *tip_hash = hash;

        Ok(Block {
            node_public_key: [0; 128],
            previous_hash: previous_hash,
            nonce: proto_block.nonce,
            transaction: proto_block.transaction,
        })
    } else {
        Err(ProtoBlock {
            nonce: to_32bytes(
                &(BigUint::from_bytes_le(&proto_block.nonce) + BigUint::from(1u32)).to_bytes_le(),
            ),
            transaction: proto_block.transaction,
        })
    }
}

async fn fetch_block_from_network() -> Option<Block> {
    None
}

async fn fetch_transaction_from_network() -> Option<Transaction> {
    None
}

async fn block_generator(
    pending: &mut VecDeque<ProtoBlock>,
    tip_hash: &mut [u8; 32],
    public_key: &[u8; 128],
) {
    match pending.pop_front() {
        Some(proto_block) => match proof_of_work(proto_block, tip_hash, public_key).await {
            Ok(block) => block_created(block).await,
            Err(proto_block) => pending.push_back(proto_block),
        },
        None => {}
    }
}

async fn block_replicator(blockchain: &mut Blockchain, tip_hash: &mut [u8; 32]) {
    match fetch_block_from_network().await {
        Some(block) => _block_received(block, blockchain, tip_hash).await,
        None => {}
    };
}

async fn transaction_replicator(pending: &mut VecDeque<ProtoBlock>) {
    match fetch_transaction_from_network().await {
        Some(transaction) => _transaction_received(transaction, pending).await,
        None => {}
    };
}

async fn async_main(
    pending: &mut VecDeque<ProtoBlock>,
    tip_hash: &mut [u8; 32],
    public_key: &[u8; 128],
    blockchain: &mut Blockchain,
) {
    // FIXME: this can probably be done in a better way with sync code and using
    // threading+queues for networking operations.

    block_generator(pending, tip_hash, public_key).await;
    transaction_replicator(pending).await;
    block_replicator(blockchain, tip_hash).await;
}

fn will_be_main() {
    let (tx, rx) = channel();

    ctrlc::set_handler(move || tx.send(()).expect("Error sending stop signal."))
        .expect("Error setting ctrl-c handler.");

    println!("fcoin");

    let public_key = read_public_key_from_disk();
    let mut blockchain: Blockchain = HashMap::new();

    let mut tip_hash: [u8; 32] = [0; 32];
    let mut pending: VecDeque<ProtoBlock> = VecDeque::new();

    loop {
        block_on(async_main(
            &mut pending,
            &mut tip_hash,
            &public_key,
            &mut blockchain,
        ));

        match rx.try_recv() {
            Ok(_) => {
                println!("Stopping fcoin");
                break;
            }
            Err(_) => {}
        }
    }
}
