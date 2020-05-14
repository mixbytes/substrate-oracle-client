extern crate log;

use sp_core::crypto::Pair;
use sp_runtime::MultiSignature;

use keyring::AccountKeyring;
use std::sync::mpsc::channel;

use codec::Decode;
use node_primitives::AccountId;

use substrate_api_client::{compose_extrinsic, extrinsic::xt_primitives::*, Api};

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

pub const ORACLE_MODULE: &str = "Oracle";
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

fn main() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .try_init();

    let url = get_local_test_node();

    let from = AccountKeyring::Alice.pair();
    let api = Api::new(format!("ws://{}", url)).set_signer(from.clone());

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

    let tx = api.send_extrinsic(xt);

    println!("{:?}", tx);

    let args: OracleCreatedArgs = api
        .wait_for_event(ORACLE_MODULE, ORACLE_CREATED_EVENT, &events_out)
        .unwrap()
        .unwrap();

    println!("{:?}", args);
}
