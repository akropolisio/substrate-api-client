/*
   Copyright 2019 Supercomputing Systems AG

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.

*/

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use sp_std::prelude::*;

#[cfg(feature = "std")]
use std::sync::mpsc::Sender as ThreadOut;
#[cfg(feature = "std")]
use std::sync::mpsc::{channel, Receiver};

#[cfg(feature = "std")]
use std::convert::TryFrom;

#[cfg(feature = "std")]
use balances::AccountData as AccountDataGen;
#[cfg(feature = "std")]
use codec::{Decode, Encode, Error as CodecError};
#[cfg(feature = "std")]
use system::AccountInfo as AccountInfoGen;

#[cfg(feature = "std")]
use log::{debug, error, info};

#[cfg(feature = "std")]
use metadata::RuntimeMetadataPrefixed;
#[cfg(feature = "std")]
use sp_core::crypto::Pair;

#[cfg(feature = "std")]
use ws::Result as WsResult;

#[cfg(feature = "std")]
use node_metadata::Metadata;

#[cfg(feature = "std")]
use rpc::json_req;

#[cfg(feature = "std")]
use utils::*;

#[cfg(feature = "std")]
use sp_version::RuntimeVersion;

#[macro_use]
pub mod extrinsic;
#[cfg(feature = "std")]
pub mod events;
#[cfg(feature = "std")]
pub mod node_metadata;

#[cfg(feature = "std")]
pub mod rpc;
#[cfg(feature = "std")]
pub mod utils;

#[cfg(feature = "std")]
use events::{EventsDecoder, RawEvent, RuntimeEvent};

use node_runtime::{AccountId, Signature};
pub use sp_core::H256 as Hash;
/// The block number type used in this runtime.
pub type BlockNumber = u64;
/// The timestamp moment type used in this runtime.
pub type Moment = u64;
/// Index of a transaction.
//fixme: make generic
pub type Index = u32;

#[cfg(feature = "std")]
pub use rpc::XtStatus;

//fixme: make generic
pub type Balance = u128;

pub type AccountData = AccountDataGen<Balance>;
pub type AccountInfo = AccountInfoGen<Index, AccountData>;

#[cfg(feature = "std")]
#[derive(Clone)]
pub struct Api<P>
where
    P: Pair,
    Signature: From<P::Signature>,
{
    url: String,
    pub signer: Option<P>,
    pub genesis_hash: Hash,
    pub metadata: Metadata,
    pub runtime_version: RuntimeVersion,
}

#[cfg(feature = "std")]
impl<P> Api<P>
where
    P: Pair,
    Signature: From<P::Signature>,
{
    pub fn new(url: String) -> Self {
        let genesis_hash = Self::_get_genesis_hash(url.clone());
        info!("Got genesis hash: {:?}", genesis_hash);

        let meta = Self::_get_metadata(url.clone());
        let metadata = Metadata::try_from(meta).unwrap();
        debug!("Metadata: {:?}", metadata);

        let runtime_version = Self::_get_runtime_version(url.clone());
        info!("Runtime Version: {:?}", runtime_version);

        Self {
            url,
            signer: None,
            genesis_hash,
            metadata,
            runtime_version,
        }
    }

    pub fn set_signer(mut self, signer: P) -> Self {
        self.signer = Some(signer);
        self
    }

    fn _get_genesis_hash(url: String) -> Hash {
        let jsonreq = json_req::chain_get_genesis_hash();
        let genesis_hash_str = Self::_get_request(url, jsonreq.to_string())
            .expect("Fetching genesis hash from node failed");
        hexstr_to_hash(genesis_hash_str).unwrap()
    }

    fn _get_runtime_version(url: String) -> RuntimeVersion {
        let jsonreq = json_req::state_get_runtime_version();
        let version_str = Self::_get_request(url, jsonreq.to_string()).unwrap(); //expect("Fetching runtime version from node failed");
        debug!("got the following runtime version (raw): {}", version_str);
        serde_json::from_str(&version_str).unwrap()
    }

    fn _get_metadata(url: String) -> RuntimeMetadataPrefixed {
        let jsonreq = json_req::state_get_metadata();
        let metadata_str = Self::_get_request(url, jsonreq.to_string()).unwrap();

        let _unhex = hexstr_to_vec(metadata_str).unwrap();
        let mut _om = _unhex.as_slice();
        RuntimeMetadataPrefixed::decode(&mut _om).unwrap()
    }

    // low level access
    fn _get_request(url: String, jsonreq: String) -> WsResult<String> {
        let (result_in, result_out) = channel();
        rpc::get(url, jsonreq, result_in);

        Ok(result_out.recv().unwrap())
    }

    pub fn get_metadata(&self) -> RuntimeMetadataPrefixed {
        Self::_get_metadata(self.url.clone())
    }

    pub fn get_spec_version(&self) -> u32 {
        Self::_get_runtime_version(self.url.clone()).spec_version
    }

    pub fn get_genesis_hash(&self) -> Hash {
        Self::_get_genesis_hash(self.url.clone())
    }

    pub fn get_nonce(&self) -> Result<u32, &str> {
        match &self.signer {
            Some(pair) => {
                let mut arr: [u8; 32] = Default::default();
                arr.clone_from_slice(pair.to_owned().public().as_ref());
                let accountid: AccountId = Decode::decode(&mut &arr.encode()[..]).unwrap();
                if let Some(info) = self.get_account_info(&accountid) {
                    Ok(info.nonce)
                } else {
                    Ok(0)
                }
            }
            None => Err("Can't get )nonce when no signer is set"),
        }
    }

    pub fn get_account_info(&self, address: &AccountId) -> Option<AccountInfo> {
        let _id: &[u8; 32] = address.as_ref();
        let storagekey: sp_core::storage::StorageKey = self
            .metadata
            .module("System")
            .unwrap()
            .storage("Account")
            .unwrap()
            .get_map::<AccountId, AccountInfo>()
            .unwrap()
            .key(address.clone());
        info!("storagekey {:?}", storagekey);
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey.0)
    }

    pub fn get_account_data(&self, address: &AccountId) -> Option<AccountData> {
        if let Some(info) = self.get_account_info(address) {
            Some(info.data)
        } else {
            None
        }
    }

    pub fn get_finalized_head(&self) -> WsResult<String> {
        Self::_get_request(
            self.url.clone(),
            json_req::chain_get_finalized_head().to_string(),
        )
    }

    pub fn get_header(&self, hash: Option<Hash>) -> WsResult<String> {
        Self::_get_request(
            self.url.clone(),
            json_req::chain_get_header(hash).to_string(),
        )
    }

    pub fn get_block(&self, hash: Option<Hash>) -> WsResult<String> {
        Self::_get_request(
            self.url.clone(),
            json_req::chain_get_block(hash).to_string(),
        )
    }

    pub fn get_request(&self, jsonreq: String) -> WsResult<String> {
        Self::_get_request(self.url.clone(), jsonreq)
    }

    pub fn get_storage_value<V: Decode + Clone>(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
    ) -> Option<V> {
        let storagekey: sp_core::storage::StorageKey = self
            .metadata
            .module(storage_prefix)
            .unwrap()
            .storage(storage_key_name)
            .unwrap()
            .get_value()
            .unwrap()
            .key();
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey.0)
    }

    pub fn get_storage_map<K: Encode, V: Decode + Clone>(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
        map_key: K,
    ) -> Option<V> {
        let storagekey: sp_core::storage::StorageKey = self
            .metadata
            .module(storage_prefix)
            .unwrap()
            .storage(storage_key_name)
            .unwrap()
            .get_map::<K, V>()
            .unwrap()
            .key(map_key);
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey.0)
    }

    pub fn get_storage_by_key_hash<V: Decode + Clone>(&self, hash: Vec<u8>) -> Option<V> {
        let mut keyhash_str = hex::encode(hash);
        keyhash_str.insert_str(0, "0x");
        let jsonreq = json_req::state_get_storage(&keyhash_str);
        if let Ok(hexstr) = Self::_get_request(self.url.clone(), jsonreq.to_string()) {
            info!("storage hex = {}", hexstr);
            let hexstr = hexstr
                .trim_matches('\"')
                .to_string()
                .trim_start_matches("0x")
                .to_string();
            match hexstr.as_str() {
                "null" => None,
                _ => Some(Decode::decode(&mut &hex::decode(&hexstr).unwrap()[..]).unwrap()),
            }
        } else {
            None
        }
    }

    pub fn get_storage_double_map<K: Encode, Q: Encode, V: Decode + Clone>(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
        first: K,
        second: Q,
    ) -> Option<V> {
        let storagekey: sp_core::storage::StorageKey = self
            .metadata
            .module(storage_prefix)
            .unwrap()
            .storage(storage_key_name)
            .unwrap()
            .get_double_map::<K, Q, V>()
            .unwrap()
            .key(first, second);
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey.0)
    }

    pub fn send_extrinsic(
        &self,
        xthex_prefixed: String,
        exit_on: XtStatus,
    ) -> WsResult<Option<Hash>> {
        debug!("sending extrinsic: {:?}", xthex_prefixed);

        let jsonreq = json_req::author_submit_and_watch_extrinsic(&xthex_prefixed).to_string();

        let (result_in, result_out) = channel();
        match exit_on {
            XtStatus::Finalized => {
                rpc::send_extrinsic_and_wait_until_finalized(
                    self.url.clone(),
                    jsonreq.clone(),
                    result_in.clone(),
                );
                let res = result_out.recv().unwrap();
                info!("finalized: {}", res);
                Ok(Some(hexstr_to_hash(res).unwrap()))
            }
            XtStatus::Ready => {
                rpc::send_extrinsic(self.url.clone(), jsonreq.clone(), result_in.clone());
                let res = result_out.recv().unwrap();
                info!("ready: {}", res);
                Ok(None)
            }
            _ => panic!("can only wait for finalized or ready extrinsic status"),
        }
    }

    pub fn subscribe_events(&self, sender: ThreadOut<String>) {
        debug!("subscribing to events");
        let key = storage_key_hash("System", "Events", None);
        let jsonreq = json_req::state_subscribe_storage(&key).to_string();

        rpc::start_event_subscriber(self.url.clone(), jsonreq, sender);
    }

    pub fn wait_for_event<E: Decode>(
        &self,
        module: &str,
        variant: &str,
        receiver: &Receiver<String>,
    ) -> Option<Result<E, CodecError>> {
        self.wait_for_raw_event(module, variant, receiver)
            .map(|raw| E::decode(&mut &raw.data[..]))
    }

    pub fn wait_for_raw_event(
        &self,
        module: &str,
        variant: &str,
        receiver: &Receiver<String>,
    ) -> Option<RawEvent> {
        loop {
            let event_str = receiver.recv().unwrap();

            let _unhex = hexstr_to_vec(event_str).unwrap();
            let mut _er_enc = _unhex.as_slice();

            let event_decoder = EventsDecoder::try_from(self.metadata.clone()).unwrap();
            let _events = event_decoder.decode_events(&mut _er_enc);
            info!("wait for raw event");
            match _events {
                Ok(raw_events) => {
                    for (phase, event) in raw_events.into_iter() {
                        info!("Decoded Event: {:?}, {:?}", phase, event);
                        match event {
                            RuntimeEvent::Raw(raw)
                                if raw.module == module && raw.variant == variant =>
                            {
                                return Some(raw)
                            }
                            _ => debug!("ignoring unsupported module event: {:?}", event),
                        }
                    }
                }
                Err(_) => error!("couldn't decode event record list"),
            }
        }
    }
}
