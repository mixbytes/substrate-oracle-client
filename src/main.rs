#![feature(result_flattening)]
extern crate log;

use sp_core::crypto::Pair;
use sp_runtime::MultiSignature;
use std::{
    convert::TryFrom,
    sync::mpsc::{channel, Receiver},
};

use keyring::AccountKeyring;

use codec::Decode;
use node_primitives::AccountId;
use substrate_api_client::node_metadata::Metadata;
use substrate_api_client::{
    compose_extrinsic,
    events::{EventsDecoder, RawEvent, RuntimeEvent},
    extrinsic::xt_primitives::*,
    utils::hexstr_to_vec,
    Api,
};

pub type AssetId = u32;
pub type OracleId = u32;
pub type Moment = u64;
pub type CallIndex = [u8; 2];

pub type CreateOracleFn = (
    CallIndex,
    Vec<u8>,
    u8,
    Moment,
    Moment,
    AssetId,
    Vec<Vec<u8>>,
);

pub type CreateOracleXt = UncheckedExtrinsicV4<CreateOracleFn>;

pub const ORACLE_MODULE: &str = "OracleModule";
pub const ORACLE_STORAGE: &str = "OracleModule";
pub const ORACLE_CREATE: &str = "create_oracle";
pub const ORACLE_CREATED_EVENT: &str = "OracleCreated";
pub const ORACLE_SEQUENCE: &str = "OracleIdSequence";

trait OracleModule {
    fn create_oracle(
        &self,
        name: Vec<u8>,
        source_limit: u8,
        period: Moment,
        aggregate_period: Moment,
        asset_id: AssetId,
        value_names: Vec<Vec<u8>>,
    ) -> CreateOracleXt;
}

impl<P> OracleModule for Api<P>
where
    P: Pair,
    MultiSignature: From<P::Signature>,
{
    fn create_oracle(
        &self,
        name: Vec<u8>,
        source_limit: u8,
        period: Moment,
        aggregate_period: Moment,
        asset_id: AssetId,
        value_names: Vec<Vec<u8>>,
    ) -> CreateOracleXt {
        compose_extrinsic!(
            self,
            ORACLE_MODULE,
            ORACLE_CREATE,
            name,
            source_limit,
            period,
            aggregate_period,
            asset_id,
            value_names
        )
    }
}

pub fn get_local_test_node() -> String {
    String::from("0.0.0.0:9999")
}

#[derive(Decode, Debug)]
struct OracleCreatedArgs {
    oracle: OracleId,
    creater: AccountId,
}

impl std::fmt::Display for OracleCreatedArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Oracle id: {}, Created by: {}",
            self.oracle, self.creater
        )
    }
}

trait WaitForCustomEvent {
    fn wait_for_custom_event<E: Decode>(
        &self,
        module: &str,
        variant: &str,
        receiver: &Receiver<String>,
    ) -> Result<E, String>;

    fn wait_for_raw_custom_event(
        &self,
        module: &str,
        variant: &str,
        receiver: &Receiver<String>,
    ) -> Result<RawEvent, String>;
}

impl<P> WaitForCustomEvent for Api<P>
where
    P: Pair,
    MultiSignature: From<P::Signature>,
{
    fn wait_for_custom_event<E: Decode>(
        &self,
        module: &str,
        variant: &str,
        receiver: &Receiver<String>,
    ) -> Result<E, String> {
        self.wait_for_raw_custom_event(module, variant, receiver)
            .map(|raw| E::decode(&mut &raw.data[..]).map_err(|err| err.to_string()))
            .flatten()
    }

    fn wait_for_raw_custom_event(
        &self,
        module: &str,
        variant: &str,
        receiver: &Receiver<String>,
    ) -> Result<RawEvent, String> {
        loop {
            let unhex = hexstr_to_vec(receiver.recv().map_err(|err| err.to_string())?)
                .map_err(|err| err.to_string())?;

            let mut event_decoder = EventsDecoder::try_from(self.metadata.clone()).unwrap();
            event_decoder
                .register_type_size::<OracleId>("OracleId")
                .unwrap(); // All DRY-violation (from client code) for this line

            match event_decoder.decode_events(&mut unhex.as_slice()) {
                Ok(raw_events) => {
                    for (_phase, event) in raw_events.into_iter() {
                        match event {
                            RuntimeEvent::Raw(raw)
                                if raw.module == module && raw.variant == variant =>
                            {
                                return Ok(raw)
                            }
                            _ => log::debug!("ignoring unsupported module event: {:?}", event),
                        }
                    }
                }
                Err(_) => log::error!("couldn't decode event record list"),
            }
        }
    }
}

fn main() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .try_init();

    let url = get_local_test_node();

    let from = AccountKeyring::Alice.pair();
    let api = Api::new(format!("ws://{}", url)).set_signer(from.clone());

    // print full substrate metadata json formatted
    println!(
        "{}",
        Metadata::pretty_format(&api.get_metadata()).unwrap_or("pretty format failed".to_string())
    );

    let (events_in, events_out) = channel();
    api.subscribe_events(events_in.clone());

    let id = api.get_storage(ORACLE_STORAGE, ORACLE_SEQUENCE, None);
    println!("{:?}", id);
    let xt = api
        .create_oracle(
            "test".to_owned().into_bytes(), // name
            5,                              // source_limit
            10,                             // period
            5,                              // aggregate_period
            1,                              // asset_id
            vec!["USD/RUB", "EUR/USD"]
                .into_iter()
                .map(|s| s.to_owned().into_bytes())
                .collect(),
        )
        .hex_encode();

    assert!(api.send_extrinsic(xt).is_ok());

    let args: Result<OracleCreatedArgs, String> =
        api.wait_for_custom_event(ORACLE_MODULE, ORACLE_CREATED_EVENT, &events_out);

    match args {
        Ok(event) => println!("{}!", event),
        Err(err) => println!("Oracle event decode failed with error {}", err),
    };
}
