//This file holds most of the peer-to-peer logic : I, Jad, am not the best at this kind of stuff 
//Thankfully my friend Ahmed is a magician and basically coached me through this all so thanks to him but my explanations may be a bit cringe 

use libp2p::{floodsub::{FloodsubEvent, Topic}, swarm::{NetworkBehaviourEventProcess, NetworkBehaviour}, futures::future::Lazy, PeerId, identity};
use serde::{Serialize, Deserialize};


//We define a key pair and a derived peer ID. Those are simply libp2p’s intrinsics for identifying a client on the network
pub static KEYS: Lazy = Lazy::new(identity::Keypair::generate_ed25519);
pub static PEER_ID: Lazy = Lazy::new(|| PeerId::from(KEYS.public()));
pub static CHAIN_TOPIC: Lazy = Lazy::new(|| Topic::new("chains"));
pub static BLOCK_TOPIC: Lazy = Lazy::new(|| Topic::new("blocks"));

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainResponse {
    pub blocks: Vec,
    pub receiver: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalChainRequest {
    pub from_peer_id: String,
}

pub enum EventType {
    LocalChainResponse(ChainResponse),
    Input(String),
    Init,
}

//We’ll use the FloodSub protocol, a simple publish/subscribe protocol, for communication between the nodes
//This is no problem in terms of correctness, but in terms of efficiency, it’s obviously horrendous. 
//This could be handled by a simple point-to-point request/response model, which is something libp2p supports, 
//but this would simply add even more complexity (and as I said, i'm not that good) to this already complex (in my opinion) practice. If you’re interested, you can check out the libp2p docs.
#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub response_sender: mpsc::UnboundedSender,
    #[behaviour(ignore)]
    pub init_sender: mpsc::UnboundedSender,
    #[behaviour(ignore)]
    pub app: App,
}

//The AppBehaviour holds our FloodSub instance for pub/sub communication and and Mdns instance, 
//which will enable us to automatically find other nodes on our local network (but not outside of it which is logical)

//We also add our blockchain App to this behaviour, 
//as well as channels for sending events for both initialization and request/response communication between parts of the app. 

impl AppBehaviour {
    pub async fn new(
        app: App,
        response_sender: mpsc::UnboundedSender,
        init_sender: mpsc::UnboundedSender,
    ) -> Self {
        let mut behaviour = Self {
            app,
            floodsub: Floodsub::new(*PEER_ID),
            mdns: Mdns::new(Default::default())
                .await
                .expect("can create mdns"),
            response_sender,
            init_sender,
        };
        behaviour.floodsub.subscribe(CHAIN_TOPIC.clone());
        behaviour.floodsub.subscribe(BLOCK_TOPIC.clone());

        behaviour
    }
}
//I don't understand THIS specific part, so I'll circle back to it in a few days and change this comment
impl NetworkBehaviourEventProcess for AppBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice::(&msg,.data) {
                if resp.receiver == PEER_ID.to_string() {
                    info!("Response from {}:", msg.source);
                    resp.blocks.iter().for_each(|r| info!("{:?}", r));

                    self.app.blocks = self.app.choose_chain(self.app.blocks.clone(), resp.blocks);
                }
            } else if let Ok(resp) = serde_json::from_slice::(&msg.data) {
                info!("sending local chain to {}", msg.source.to_string());
                let peer_id = resp.from_peer_id;
                if PEER_ID.to_string() == peer_id {
                    if let Err(e) = self.response_sender.send(ChainResponse {
                        blocks: self.app.blocks.clone(),
                        receiver: msg.source.to_string(),
                    }) {
                        error!("error sending response via channel, {}", e);
                    }
                }
            } else if let Ok(block) = serde_json::from_slice::(&msg.data) {
                info!("received new block from {}", msg.source.to_string());
                self.app.try_add_block(block);
            }
        }
    }
}