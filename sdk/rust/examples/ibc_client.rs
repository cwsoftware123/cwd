use {
    cw_ibc_solomachine::{
        ClientState, ConsensusState, Header, Misbehavior, QueryMsg, Record, SignBytes,
        StateResponse,
    },
    cw_rs::{Client, SigningKey, SigningOptions},
    cw_std::{hash, to_borsh, to_json, Addr, Hash, IbcClientStatus, StdResult},
    hex_literal::hex,
    home::home_dir,
    lazy_static::lazy_static,
    std::{env, path::PathBuf, thread, time::Duration},
};

lazy_static! {
    static ref ARTIFACT_DIR: PathBuf = {
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../artifacts")
    };
    static ref KEYSTORE_DIR: PathBuf = {
        home_dir().unwrap().join(".cwcli/keys")
    };
}

const USER: Addr = Addr::from_slice(hex!("5f93cc3ed709beb4d0b105d43f65818fafc943cb10adc06f4f82cce82313069d"));

const SOLOMACHINE_HASH: Hash = Hash::from_slice(hex!("57af38f120183d53b4e2cc0c7f98e8c0fee1982f4d998a7b3880fbce1525ea12"));

const KEYSTORE_PASSWORD: &str = "123";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // load signing key
    let test1 = SigningKey::from_file(&KEYSTORE_DIR.join("test1.json"), KEYSTORE_PASSWORD)?;
    let sign_opts = SigningOptions {
        signing_key: test1.clone(),
        sender:      USER.clone(),
        chain_id:    None,
        sequence:    None,
    };

    // create cw-rs client
    let client = Client::connect("http://127.0.0.1:26657")?;

    // ----------------------------- create client -----------------------------

    // create a new IBC client using the solo machine code hash
    let salt = b"06-solomachine-0".to_vec().into();
    let (address, tx1) = client.create_client(
        SOLOMACHINE_HASH,
        &ClientState {
            status: IbcClientStatus::Active,
        },
        &ConsensusState {
            public_key: test1.public_key().to_vec().into(),
            sequence: 0,
            record: None,
        },
        salt,
        &sign_opts,
    )
    .await?;
    println!("\nCreating IBC client...");
    println!("address: {}", address);
    println!("txhash: {}", tx1.hash);

    // wait 1 second for tx to settle
    thread::sleep(Duration::from_secs(1));

    // query the client's state
    query_client_state(&client, &address).await?;

    // ----------------------------- update client -----------------------------

    // sign a header and update client state
    let header = create_header(b"foo", b"bar", 0, &test1)?;
    let tx2 = client.update_client(address.clone(), to_json(&header)?, &sign_opts).await?;
    println!("\nUpdating IBC client...");
    println!("txhash: {}", tx2.hash);

    // wait 1 second for tx to settle
    thread::sleep(Duration::from_secs(1));

    // query the client's state again
    query_client_state(&client, &address).await?;

    // -------------------------- submit misbehavior ---------------------------

    // sign two headers at the same sequence and submit misbehavior
    let header_one = create_header(b"foo", b"bar", 1, &test1)?;
    let header_two = create_header(b"fuzz", b"buzz", 1, &test1)?;
    let misbehavior = Misbehavior {
        sequence: 1,
        header_one,
        header_two,
    };
    let tx3 = client.submit_misbehavior(address.clone(), to_json(&misbehavior)?, &sign_opts).await?;
    println!("\nSubmitting misbehavior...");
    println!("txhash: {}", tx3.hash);

    // wait 1 second for tx to settle
    thread::sleep(Duration::from_secs(1));

    // query the client's state again
    query_client_state(&client, &address).await?;

    Ok(())
}

async fn query_client_state(client: &Client, address: &Addr) -> anyhow::Result<()> {
    let state_res: StateResponse = client.query_wasm_smart(
        address.clone(),
        &QueryMsg::State {},
        None,
    )
    .await?;
    println!("\n{}", serde_json::to_string_pretty(&state_res)?);
    Ok(())
}

fn create_header(key: &[u8], value: &[u8], sequence: u64, sk: &SigningKey) -> StdResult<Header> {
    let record = Some(Record {
        key: key.to_vec().into(),
        value: value.to_vec().into(),
    });
    let sign_bytes = SignBytes {
        sequence,
        record: record.clone(),
    };
    let sign_bytes_hash = hash(to_borsh(&sign_bytes)?);
    let signature = sk.sign_digest(&sign_bytes_hash.into_slice());
    Ok(Header {
        signature: signature.into(),
        record,
    })
}
