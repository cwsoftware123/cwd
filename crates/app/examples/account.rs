//! How to run this example:
//!
//! $ just optimize
//! $ cargo run -p cw-app --example account

use {
    cw_account::{sign_bytes, InstantiateMsg, PubKey, QueryMsg, StateResponse},
    cw_app::App,
    cw_std::{from_json, hash, to_json, Addr, GenesisState, Message, MockStorage, Query, Storage, Tx},
    k256::ecdsa::{SigningKey, VerifyingKey, signature::DigestSigner},
    rand::rngs::OsRng,
    serde::{de::DeserializeOwned, ser::Serialize},
    std::{env, fs::File, io::Read, path::PathBuf},
};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    println!("🤖 Creating app");
    let mut app = App::new(MockStorage::new());

    println!("🤖 Reading wasm byte code from file");
    let artifacts_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../../artifacts");
    let wasm_file_path = {
        #[cfg(target_arch = "aarch64")]
        { artifacts_dir.join("cw_account-aarch64.wasm") }
        #[cfg(not(target_arch = "aarch64"))]
        { artifacts_dir.join("cw_account.wasm") }
    };
    let mut wasm_file = File::open(wasm_file_path)?;
    let mut wasm_byte_code = Vec::new();
    wasm_file.read_to_end(&mut wasm_byte_code)?;

    println!("🤖 Generate two random secp256k1 key pairs");
    let sk1 = SigningKey::random(&mut OsRng);
    let vk1 = VerifyingKey::from(&sk1);
    let sk2 = SigningKey::random(&mut OsRng);
    let vk2 = VerifyingKey::from(&sk2);

    println!("🤖 Computing account addresses");
    let code_hash = hash(&wasm_byte_code);
    let salt1 = b"account-1".to_vec().into();
    let salt2 = b"account-2".to_vec().into();
    // note: we use a zeroed-out address as sender during genesis
    let address1 = Addr::compute(&Addr::mock(0), &code_hash, &salt1);
    let address2 = Addr::compute(&Addr::mock(0), &code_hash, &salt2);

    println!("🤖 Genesis chain, instantiate accounts 1");
    app.init_chain(GenesisState {
        chain_id: "dev-1".to_string(),
        msgs: vec![
            Message::StoreCode {
                wasm_byte_code: wasm_byte_code.into(),
            },
            Message::Instantiate {
                code_hash,
                msg: to_json(&InstantiateMsg {
                    pubkey: PubKey::Secp256k1(vk1.to_sec1_bytes().to_vec().into()),
                })?,
                salt:  salt1,
                funds: vec![],
                admin: None,
            },
        ],
    })?;

    println!("🤖 Account 1 sends a tx, creating account 2");


    println!("🤖 Querying chain info");
    query(&mut app, Query::Info {})?;

    println!("🤖 Querying codes");
    query(&mut app, Query::Codes {
        start_after: None,
        limit:       None,
    })?;

    println!("🤖 Querying accounts");
    query(&mut app, Query::Accounts {
        start_after: None,
        limit:       None,
    })?;

    println!("🤖 Querying account state");
    query_wasm_smart::<_, _, StateResponse>(&mut app, &address, &QueryMsg::State {})?;

    Ok(())
}

fn mock_block_info(height: u64, timestamp: u64) -> BlockInfo {
    BlockInfo {
        chain_id: "dev-1".into(),
        height,
        timestamp,
    }
}

fn new_tx<S: Storage>(
    app:    &mut App<S>,
    sender: &Addr,
    sk:     &SigningKey,
    msgs:   Vec<Message>,
) -> anyhow::Result<Tx> {
    // query chain_id
    let chain_id = app
        .query(Query::Info {})?
        .as_info()
        .chain_id;

    // query account sequence
    let sequence = from_json::<StateResponse>(app
        .query(Query::WasmSmart {
            contract: sender.clone(),
            msg: to_json(&QueryMsg::State {})?,
        })?
        .as_wasm_smart()
        .data)?
        .sequence;

    // create sign bytes
    let sign_bytes = sign_bytes(&msgs, sender, &chain_id, sequence)?;

    // sign the sign bytes
    let signature = sk.sign_digest(&sign_bytes);

    Ok(Tx {
        sender: sender.clone(),
        credential: signature,
        msgs,
    })
}

fn query<S>(app: &mut App<S>, req: Query) -> anyhow::Result<()>
where
    S: Storage + 'static,
{
    let resp = app.query(req)?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

fn query_wasm_smart<S, M, T>(app: &mut App<S>, contract: &Addr, msg: &M) -> anyhow::Result<()>
where
    S: Storage + 'static,
    M: Serialize,
    T: Serialize + DeserializeOwned,
{
    let resp = app.query(Query::WasmSmart {
        contract: contract.clone(),
        msg: to_json(msg)?,
    })?;
    let resp: T = from_json(resp.as_wasm_smart().data)?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}
