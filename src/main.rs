mod p2p;

use chrono::Utc;
use log::info;
use serde::{Serialize, Deserialize};
use sha2::Sha256;

// We need to define the data structure for our actual blockchain 
// Not much behind it : App => Application state and considering I won't persist the blockchain in this practice
//                             it will go away once the application is stopped
// This state is simply a list of Blocks. 
// We will add new blocks to the end of this list and this will actually be our blockchain data structure.
pub struct App {
    pub blocks: Vec,
}
// This struct represents the actual block itself
// It would be possible to build a data structure that already supports the validation we need out of the box
// but this approach seems simpler (I'll probably implement it in another practice trial)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub id: u64,
    pub hash: String,
    pub previous_hash: String,
    pub timestamp: i64,
    pub data: String,
    pub nonce: u64,
}

//For simplicity’s sake, I’ll just hardcode it to two leading zeros
//This doesn’t take too long to compute on normal hardware, so there's no need to worry about waiting too long when testing
const DIFFICULTY_PREFIX: &str = "00";

fn hash_to_binary_representation(hash: &[u8]) -> String {
    let mut res: String = String::default();
    for c in hash {
        res.push_str(&format!("{:b}", c));
    }
    res
}


impl App{
    impl App {
        //We initialize our application with an empty chain
        //considering we’ll implement some logic later on 
        //There's a simplistic consensus criteria at hand here : 
        //we ask other nodes on startup for their chain and, if its longer than ours, use theirs
        fn new() -> Self {
            Self { blocks: vec![] }
        }
    
        //This method simply creates the genesis block which explains the lack of a concrete and valid previous_hash
        fn genesis(&mut self) {
            let genesis_block = Block {
                id: 0,
                timestamp: Utc::now().timestamp(),
                previous_hash: String::from("genesis"),
                data: String::from("genesis!"),
                nonce: 2836,
                hash: "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(),
            };
            self.blocks.push(genesis_block);
        }

        //Now we get to add blocks we fetch the last block in the chain — our previous block — 
        //and then validate whether the block we’d like to add is actually valid. If not, we simply log an error.
        //I won't implement any actual and concrete error handling here since if I run into any problems with race-conditions between
        //the nodes resulting in an invalid state, the node will just be broken (at least I think)
        fn try_add_block(&mut self, block: Block) {
            let latest_block = self.blocks.last().expect("there is at least one block");
            if self.is_block_valid(&block, latest_block) {
                self.blocks.push(block);
            } else {
                error!("could not add block - invalid");
            }
        }

        //Now to the logic of validating a Block. This is important because it ensures our blockchain adheres to it’s chain property and is hard to tamper with
        //The difficulty of changing something increases with every block
        // since you’d have to recalculate (which basically means re-mine) the rest of the chain to get a valid chain again. 
        //(this would be expensive enough to disincentivise you in a real blockchain system)

        //The approach I took to validate a block is not very optimized and far from being sophisticated since I don't even have a retry mechanism 
        //But I will ensure said approach will work for my local test network (or yours)
        fn is_block_valid(&self, block: &Block, previous_block: &Block) -> bool {
            if block.previous_hash != previous_block.hash {
                warn!("block with id: {} has wrong previous hash", block.id);
                return false;
            } else if !hash_to_binary_representation(
                &hex::decode(&block.hash).expect("can decode from hex"),
            )
            .starts_with(DIFFICULTY_PREFIX)
            {
                warn!("block with id: {} has invalid difficulty", block.id);
                return false;
            } else if block.id != previous_block.id + 1 {
                warn!(
                    "block with id: {} is not the next block after the latest: {}",
                    block.id, previous_block.id
                );
                return false;
            } else if hex::encode(calculate_hash(
                block.id,
                block.timestamp,
                &block.previous_hash,
                &block.data,
                block.nonce,
            )) != block.hash
            {
                warn!("block with id: {} has invalid hash", block.id);
                return false;
            }
            true
        }
        //Ignoring the genesis block, we basically just go through all the blocks and validate them
        //If one block fails the validation, we fail the whole chain. Pretty nifty isn't it ? 
        fn is_chain_valid(&self, chain: &[Block]) -> bool {
            for i in 0..chain.len() {
                if i == 0 {
                    continue;
                }
                let first = chain.get(i - 1).expect("has to exist");
                let second = chain.get(i).expect("has to exist");
                if !self.is_block_valid(second, first) {
                    return false;
                }
            }
            true
        }
        // We always choose the longest valid chain
        // Our criteria is simply the length of the chain. In real systems, there are usually more factors, such as the difficulty factored in and many other possibilities. 
        // For the purpose of this VERY SIMPLISTIC implementation, if a (valid) chain is longer than the other, then we take that one
        fn choose_chain(&mut self, local: Vec, remote: Vec) -> Vec {
            let is_local_valid = self.is_chain_valid(&local);
            let is_remote_valid = self.is_chain_valid(&remote);

            if is_local_valid && is_remote_valid {
                if local.len() >= remote.len() {
                    local
                } else {
                    remote
                }
            } else if is_remote_valid && !is_local_valid {
                remote
            } else if !is_remote_valid && is_local_valid {
                local
            } else {
                panic!("local and remote chains are both invalid");
            }
        }
    }
}

//When a new block is created, we call mine_block, which will return a nonce and a hash
//Then we can create the block with its timestamp, the given data, ID, previous hash, and the new hash and nonce.
impl Block {
    pub fn new(id: u64, previous_hash: String, data: String) -> Self {
        let now = Utc::now();
        let (nonce, hash) = mine_block(id, now.timestamp(), &previous_hash, &data);
        Self {
            id,
            hash,
            timestamp: now.timestamp(),
            previous_hash,
            data,
            nonce,
        }
    }
}
//Essentially, we’re desperately trying to find a piece of data — in this case, the nonce and a number, 
//which, together with our block data hashed using SHA256, will give us a hash starting with two zeros.
fn mine_block(id: u64, timestamp: i64, previous_hash: &str, data: &str) -> (u64, String) {
    info!("mining block...");
    let mut nonce = 0;

    loop {
        if nonce % 100000 == 0 {
            info!("nonce: {}", nonce);
        }
        let hash = calculate_hash(id, timestamp, previous_hash, data, nonce);
        let binary_hash = hash_to_binary_representation(&hash);
        if binary_hash.starts_with(DIFFICULTY_PREFIX) {
            info!(
                "mined! nonce: {}, hash: {}, binary hash: {}",
                nonce,
                hex::encode(&hash),
                binary_hash
            );
            return (nonce, hex::encode(hash));
        }
        nonce += 1;
    }
}
//This one is rather straightforward. We create a JSON-representation of our block data using the current nonce and put it through sha2‘s SHA256 hasher, returning a Vec<u8>.
//Simple, quick, easy
fn calculate_hash(id: u64, timestamp: i64, previous_hash: &str, data: &str, nonce: u64) -> Vec<u8> {
    let data = serde_json::json!({
        "id": id,
        "previous_hash": previous_hash,
        "data": data,
        "timestamp": timestamp,
        "nonce": nonce
    });
    let mut hasher = Sha256::new();
    hasher.update(data.to_string().as_bytes());
    hasher.finalize().as_slice().to_owned()
}