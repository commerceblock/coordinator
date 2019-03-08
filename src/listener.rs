//! Listener
//!
//! Listener interface and implementations

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin::consensus::serialize;
use bitcoin::util::hash::{BitcoinHash, Sha256dHash};
use secp256k1::{Message, PublicKey, Secp256k1, Signature};

use crate::challenger::{ChallengeResponse, ChallengeState};
use crate::error::{CError, Result};

/// Enum for all the messages handled by the listener
pub enum ListenerMsg {
    /// Variant for MsgChallengeSig
    ChallengeSig(MsgChallengeSig),
}

/// Messsage type for challenge proofs sent by guardnodes
pub struct MsgChallengeSig {
    hash: Sha256dHash,
    pubkey: PublicKey,
    sig: Signature,
}

/// Listener trait defining desired functionality for the struct that handles
/// incoming requests, verifies them and informs the challenger of the verified
/// ones via the ChallengeResponse model
pub trait Listener {
    /// Main do_work listener method listening to incoming requests, verifying
    /// and sending responses to challenger
    fn do_work(
        &self,
        challenge: Arc<Mutex<ChallengeState>>,
        vtx: Sender<ChallengeResponse>,
        trx: Receiver<()>,
    ) -> thread::JoinHandle<()>;

    /// Handle message challenge sig
    fn handle_msg_challengesig(&self, msg: MsgChallengeSig) -> Result<()> {
        let secp = Secp256k1::new();

        match secp.verify(
            &Message::from_slice(&serialize(&msg.hash)).unwrap(),
            &msg.sig,
            &msg.pubkey,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(CError::Coordinator("handle_msg_challengesig failed")),
        }
    }

    /// Handle message
    fn handle_msg(&self, msg: ListenerMsg) -> Result<()> {
        match msg {
            ListenerMsg::ChallengeSig(c) => self.handle_msg_challengesig(c),
        }
    }
}

/// Mock implementation of Listener for generating mock challenge responses
pub struct MockListener {}

/// Note
/// This is a temporary implementation for integration testing with other
/// interfaces. Ideally it will be removed once the listener interface has been
/// finalised and replaced by dummy requests sent to the listener on any
/// integration tests
impl Listener for MockListener {
    /// Run mock listener do_work method producing mock challenge responses
    fn do_work(
        &self,
        challenge: Arc<Mutex<ChallengeState>>,
        vtx: Sender<ChallengeResponse>,
        trx: Receiver<()>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            match trx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    info!("Verify ended");
                    break;
                }
                Err(TryRecvError::Empty) => {}
            }

            // get immutable lock to avoid changing any data
            let challenge_lock = challenge.lock().unwrap();

            if let Some(latest) = challenge_lock.latest_challenge {
                vtx.send(ChallengeResponse(
                    latest,
                    challenge_lock.bids.iter().next().unwrap().clone(),
                ))
                .unwrap();
            }
            std::mem::drop(challenge_lock);

            thread::sleep(time::Duration::from_millis(500))
        })
    }
}
