mod client;
// pub mod tx_stream;

pub const PRICE: u64 = 5;

use std::{
    collections::HashSet,
    string::ToString,
    sync::{Arc, RwLock},
};

use bitcoin::{Transaction, TxOut};
use serde::Deserialize;

use crate::models::Output;

pub use client::BitcoinClient;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum Network {
    Mainnet = 0,
    Testnet = 1,
    Regnet = 2,
}

impl From<bitcoincash_addr::Network> for Network {
    fn from(network: bitcoincash_addr::Network) -> Network {
        match network {
            bitcoincash_addr::Network::Main => Network::Mainnet,
            bitcoincash_addr::Network::Test => Network::Testnet,
            bitcoincash_addr::Network::Regtest => Network::Regnet,
        }
    }
}

impl Into<bitcoincash_addr::Network> for Network {
    fn into(self) -> bitcoincash_addr::Network {
        match self {
            Network::Mainnet => bitcoincash_addr::Network::Main,
            Network::Testnet => bitcoincash_addr::Network::Test,
            Network::Regnet => bitcoincash_addr::Network::Regtest,
        }
    }
}

impl ToString for Network {
    fn to_string(&self) -> String {
        match self {
            Network::Mainnet => "mainnet".to_string(),
            Network::Testnet => "testnet".to_string(),
            Network::Regnet => "regnet".to_string(),
        }
    }
}

#[derive(Default, Clone)]
pub struct WalletState(Arc<RwLock<HashSet<Vec<u8>>>>);

impl WalletState {
    pub fn add(&self, addr: Vec<u8>) {
        self.0.write().unwrap().insert(addr);
    }

    pub fn remove(&self, addr: Vec<u8>) {
        self.0.write().unwrap().remove(&addr);
    }

    pub fn check_p2pkh(&self, output: &TxOut) -> bool {
        // Check first output
        let value = output.value;
        if value != PRICE {
            return false;
        }

        // Check p2pkh addr
        let script = &output.script_pubkey[..];
        if let Some(pubkey_hash) = extract_pubkey_hash(script) {
            // Check if wallet contains that address
            if self.0.read().unwrap().contains(&pubkey_hash) {
                // Flush address
                self.0.write().unwrap().remove(&pubkey_hash);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn check_outputs(&self, tx: Transaction) -> bool {
        // TODO: Enforce op_return outputs
        tx.output.iter().any(|output| self.check_p2pkh(output))
    }
}

pub fn extract_op_return(script: &[u8]) -> Option<Vec<u8>> {
    // OP_RETURN || LEN || keyserver || bitcoin pk hash || peer host
    if script.len() <= 2 + 9 + 20 {
        // Too short
        return None;
    }

    if script[0] != 106 {
        // Not op_return
        return None;
    }

    if script[1] as usize != script.len() - 2 {
        // Not length
        return None;
    }

    Some(script[2..script.len()].to_vec())
}

fn extract_pubkey_hash(raw_script: &[u8]) -> Option<Vec<u8>> {
    if raw_script.len() != 25 {
        return None;
    }

    if raw_script[0..3] != [118, 169, 20] {
        return None;
    }

    if raw_script[23..25] != [136, 172] {
        return None;
    }

    Some(raw_script[3..23].to_vec())
}

pub fn generate_outputs(pk_hash: Vec<u8>, data: Vec<Vec<u8>>) -> Vec<Output> {
    // Generate p2pkh
    let p2pkh_script_pre: [u8; 3] = [118, 169, 20];
    let p2pkh_script_post: [u8; 2] = [136, 172];
    let p2pkh_script = [&p2pkh_script_pre[..], &pk_hash[..], &p2pkh_script_post[..]].concat();
    let p2pkh_output = Output {
        amount: Some(PRICE),
        script: p2pkh_script,
    };

    // Generate op return
    let op_returns: Vec<Output> = data
        .iter()
        .map(|item| {
            let op_return_script = [&[106, 9 + 20 + item.len() as u8][..], &item].concat();
            Output {
                amount: Some(0),
                script: op_return_script,
            }
        })
        .collect();
    [vec![p2pkh_output], op_returns].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_check_output() {
        let pk_hash = [3; 20].to_vec();
        let outputs = generate_outputs(pk_hash.clone(), "", pk_hash.clone());
        assert_eq!(PRICE, outputs.get(0).unwrap().amount.unwrap());
        let extracted_pkh = extract_pubkey_hash(&outputs.get(0).unwrap().script[..]);
        assert_eq!(pk_hash, extracted_pkh.unwrap());
    }
}
