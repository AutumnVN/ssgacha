mod credentials;
mod crypto;
mod device;
mod metadata;
mod net;
mod output;
mod proto;
mod regions;
mod sdk;
mod session;

use std::collections::BTreeSet;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use prost::Message;

use crate::credentials::{find_yosdk_credentials, region_credentials, Credentials};
use crate::metadata::{build_catalog, load_metadata, OutputError, OutputErrorKey};
use crate::net::{do_ike_handshake, with_retry, IkeResult};
use crate::output::{
    build_region_output, load_existing_region, output_file_for_account, write_region_output,
    FetchedRegion, Pull,
};
use crate::proto::{GachaHistories, GachaInformationResp, PlayerInfo, Ui32};
use crate::regions::{normalize_region, region_by_key, AuthMode, Region, REGIONS, REGION_ORDER};
use crate::sdk::{
    do_player_login, get_account_login_token, login_with_email_code, send_auth_code,
    send_game_request, QuickLogin, MSG_GACHA_HISTORIES_REQ, MSG_GACHA_HISTORIES_SUCCEED_ACK,
    MSG_GACHA_INFORMATION_REQ, MSG_GACHA_INFORMATION_SUCCEED_ACK, MSG_PLAYER_DATA_REQ,
    MSG_PLAYER_DATA_SUCCEED_ACK,
};

async fn fetch_game_identity(
    client: &reqwest::Client,
    region: &Region,
    session_token: &str,
    cipher: i32,
    session_key: &[u8],
) -> Result<(String, Option<String>)> {
    let response = with_retry(
        || {
            send_game_request(
                client,
                region,
                session_token,
                cipher,
                session_key,
                MSG_PLAYER_DATA_REQ,
                &[],
                MSG_PLAYER_DATA_SUCCEED_ACK,
            )
        },
        3,
        800,
    )
    .await?;
    let info = PlayerInfo::decode(&response[..]).context("decoding player data")?;
    let acc = info.acc.context("Player data has no account info")?;
    let game_uid = acc
        .id
        .filter(|id| *id > 0)
        .context("Player data has no game UID")?;
    let game_name = acc.nickname.filter(|name| !name.is_empty());
    Ok((game_uid.to_string(), game_name))
}

async fn fetch_region_history(
    client: &reqwest::Client,
    region: &Region,
    credentials: &Credentials,
    quick: &QuickLogin,
    metadata: &metadata::Metadata,
) -> Result<FetchedRegion> {
    let catalog = build_catalog(metadata)?;

    println!("Connecting to {}...", region.server_url);
    let ike: IkeResult = do_ike_handshake(client, region).await?;
    println!(
        "IKE ok ({})",
        if ike.cipher == 1 {
            "ChaCha20-Poly1305"
        } else {
            "AES-GCM"
        }
    );

    let session_token = do_player_login(
        client,
        region,
        credentials,
        &ike.token,
        ike.cipher,
        &ike.session_key,
        &quick.account_login_token,
        &quick.account_uid,
    )
    .await?;
    if session_token.is_empty() {
        bail!("Player login returned an empty session token");
    }

    crypto::reset_http_seq(1);

    let (game_uid, game_name) =
        fetch_game_identity(client, region, &session_token, ike.cipher, &ike.session_key).await?;
    match &game_name {
        Some(name) => println!("Name: {name} (game UID {game_uid})"),
        None => println!("Game UID {game_uid}"),
    }

    let mut storage_ids: BTreeSet<u32> = catalog.storages.keys().copied().collect();
    for banner in &catalog.banners {
        storage_ids.insert(banner.storage_id);
    }
    storage_ids = storage_ids.into_iter().filter(|id| *id > 0).collect();

    let mut pulls = Vec::new();
    let mut errors = Vec::new();

    for storage_id in storage_ids {
        let body = Ui32 {
            value: Some(storage_id),
        }
        .encode_to_vec();
        let result = with_retry(
            || {
                send_game_request(
                    client,
                    region,
                    &session_token,
                    ike.cipher,
                    &ike.session_key,
                    MSG_GACHA_HISTORIES_REQ,
                    &body,
                    MSG_GACHA_HISTORIES_SUCCEED_ACK,
                )
            },
            3,
            800,
        )
        .await;

        match result.and_then(|response| Ok(GachaHistories::decode(&response[..])?)) {
            Ok(histories) => {
                for history in histories.list {
                    pulls.push(Pull {
                        banner_id: history.gid.unwrap_or(0),
                        storage_id,
                        time: history.time.unwrap_or(0),
                        ids: history.ids,
                    });
                }
            }
            Err(error) => {
                eprintln!("  storage {storage_id}: {error}");
                errors.push(OutputError {
                    storage_id: OutputErrorKey::Number(storage_id),
                    error: error.to_string(),
                });
            }
        }
    }

    let mut gacha_info = Vec::new();
    let information_result = with_retry(
        || {
            send_game_request(
                client,
                region,
                &session_token,
                ike.cipher,
                &ike.session_key,
                MSG_GACHA_INFORMATION_REQ,
                &[],
                MSG_GACHA_INFORMATION_SUCCEED_ACK,
            )
        },
        3,
        800,
    )
    .await;

    match information_result.and_then(|response| Ok(GachaInformationResp::decode(&response[..])?)) {
        Ok(response) => gacha_info = response.information,
        Err(error) => {
            eprintln!("Gacha information failed: {error}");
            errors.push(OutputError {
                storage_id: OutputErrorKey::Text("information".to_string()),
                error: error.to_string(),
            });
        }
    }

    Ok(FetchedRegion {
        uid: quick.account_uid.clone(),
        game_uid,
        game_name,
        catalog,
        pulls,
        gacha_info,
        errors,
    })
}

async fn fetch_and_write(
    client: &reqwest::Client,
    region: &Region,
    credentials: &Credentials,
    quick: &QuickLogin,
    metadata: &metadata::Metadata,
) -> Result<u64> {
    let fetched = fetch_region_history(client, region, credentials, quick, metadata).await?;
    let output_file = output_file_for_account(region.key, &fetched.game_uid)?;
    let existing = load_existing_region(&output_file);
    let region_output =
        build_region_output(region.key, &fetched, existing.as_ref(), &metadata.item_data)?;
    write_region_output(&output_file, &region_output)?;
    println!("{}: {} pulls", region.label, region_output.totals.pulls);
    println!("Wrote {}", output_file.display());
    Ok(region_output.totals.pulls)
}

fn generated_device() -> Option<String> {
    session::configured_device()
        .or_else(|| device::get_unity_device_unique_identifier().map(str::to_string))
}

async fn try_saved_login(
    client: &reqwest::Client,
    region: &Region,
) -> Option<(Credentials, QuickLogin)> {
    let saved = session::load_saved_login(region.key)?;
    let device = if saved.device.is_empty() {
        match generated_device() {
            Some(device) => device,
            None => {
                eprintln!(
                    "Saved login failed ({}): could not determine device ID",
                    region.label
                );
                return None;
            }
        }
    } else {
        saved.device
    };
    let credentials = Credentials {
        uid: saved.uid,
        token: saved.token,
        device,
        file: PathBuf::from("<saved login>"),
        region: Some(region.key),
        mtime: 0,
        login_time: 0,
    };

    match get_account_login_token(client, region, &credentials).await {
        Ok(quick) => {
            println!(
                "Using saved login ({}, Yostar UID {})",
                region.label, quick.account_uid
            );
            Some((credentials, quick))
        }
        Err(error) => {
            eprintln!("Saved login failed ({}): {error:#}", region.label);
            None
        }
    }
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    let read = std::io::stdin().read_line(&mut line)?;
    if read == 0 {
        bail!("No input");
    }
    let value = line.trim().to_string();
    if value.is_empty() {
        bail!("Input cannot be empty");
    }
    Ok(value)
}

fn select_region() -> Result<&'static Region> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("No usable credentials. Run in a terminal to log in");
    }

    let login_regions: Vec<&Region> = REGIONS
        .iter()
        .filter(|region| region.auth_mode != AuthMode::Cn)
        .collect();

    println!("Select a region:");
    for (index, region) in login_regions.iter().enumerate() {
        println!("  {}. {}", index + 1, region.label);
    }
    let input = prompt(&format!("Region [1-{}]: ", login_regions.len()))?;
    if let Some(region) = normalize_region(&input).and_then(region_by_key) {
        if login_regions
            .iter()
            .any(|candidate| candidate.key == region.key)
        {
            return Ok(region);
        }
    }
    let index: usize = input.parse().context("Invalid region selection")?;
    login_regions
        .get(index.wrapping_sub(1))
        .copied()
        .context("Invalid region selection")
}

async fn login_with_email(
    client: &reqwest::Client,
    region: &Region,
) -> Result<(Credentials, QuickLogin)> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("Email login requires an interactive terminal");
    }

    let device = generated_device().context("Could not determine device ID")?;
    let saved_uid = session::load_saved_login(region.key)
        .map(|saved| saved.uid)
        .unwrap_or_default();
    let email = prompt(&format!("Yostar email ({}): ", region.label))?;
    println!("Requesting Yostar code...");
    send_auth_code(client, region, &email, &device, &saved_uid).await?;
    let code = prompt(&format!("Enter code ({}): ", region.label))?;
    let quick = login_with_email_code(client, region, &email, &code, &device, &saved_uid).await?;

    if let Err(error) = session::save_login(
        region.key,
        &quick.account_uid,
        &quick.account_login_token,
        &device,
    ) {
        eprintln!("Could not save login to .env: {error:#}");
    }
    println!(
        "Logged in ({}, Yostar UID {})",
        region.label, quick.account_uid
    );

    let credentials = Credentials {
        uid: quick.account_uid.clone(),
        token: quick.account_login_token.clone(),
        device,
        file: PathBuf::from("<email login>"),
        region: Some(region.key),
        mtime: 0,
        login_time: 0,
    };
    Ok((credentials, quick))
}

async fn run() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let detected: Vec<&Region> = REGION_ORDER
        .iter()
        .filter_map(|key| region_by_key(key))
        .filter(|region| {
            let has_game_credentials = find_yosdk_credentials(region.key)
                .map(|credentials| !credentials.is_empty())
                .unwrap_or(false);
            let has_saved_login = session::load_saved_login(region.key).is_some();
            has_game_credentials || has_saved_login
        })
        .collect();
    let no_regions_detected = detected.is_empty();

    let labels = detected
        .iter()
        .map(|region| region.label)
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "Detected {} region{}:{}",
        detected.len(),
        if detected.len() == 1 { "" } else { "s" },
        if labels.is_empty() {
            String::new()
        } else {
            format!(" {labels}")
        }
    );
    if no_regions_detected {
        eprintln!("No credentials detected.");
    }

    let mut success_count = 0usize;
    let mut total_pulls = 0u64;

    for region in detected {
        let accounts = region_credentials(region);
        let metadata = match load_metadata(&client, region).await {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("{} failed: {error:#}", region.label);
                continue;
            }
        };

        if accounts.is_empty() {
            let Some((credentials, quick)) = try_saved_login(&client, region).await else {
                continue;
            };
            match fetch_and_write(&client, region, &credentials, &quick, &metadata).await {
                Ok(pulls) => {
                    success_count += 1;
                    total_pulls += pulls;
                }
                Err(error) => eprintln!("{} failed: {error:#}", region.label),
            }
            continue;
        }

        let mut region_success = false;
        for credentials in accounts {
            println!(
                "Using {} ({}, Yostar UID {})",
                credentials.file.display(),
                region.label,
                credentials.uid
            );

            let region_result: Result<u64> = async {
                let quick = get_account_login_token(&client, region, credentials).await?;
                fetch_and_write(&client, region, credentials, &quick, &metadata).await
            }
            .await;

            match region_result {
                Ok(pulls) => {
                    region_success = true;
                    success_count += 1;
                    total_pulls += pulls;
                }
                Err(error) => eprintln!("{} failed: {error:#}", region.label),
            }
        }

        if !region_success && session::load_saved_login(region.key).is_some() {
            if let Some((credentials, quick)) = try_saved_login(&client, region).await {
                match fetch_and_write(&client, region, &credentials, &quick, &metadata).await {
                    Ok(pulls) => {
                        success_count += 1;
                        total_pulls += pulls;
                    }
                    Err(error) => eprintln!("{} failed: {error:#}", region.label),
                }
            }
        }
    }

    if no_regions_detected && success_count == 0 {
        let region = select_region()?;
        let (credentials, quick) = match try_saved_login(&client, region).await {
            Some(login) => login,
            None => login_with_email(&client, region).await?,
        };
        let metadata = load_metadata(&client, region)
            .await
            .with_context(|| format!("Loading {} metadata", region.label))?;
        let pulls = fetch_and_write(&client, region, &credentials, &quick, &metadata).await?;
        success_count += 1;
        total_pulls += pulls;
    }

    if success_count == 0 {
        bail!("No region could be fetched");
    }
    if total_pulls == 0 {
        eprintln!("Warning: the fetched history is empty. The metadata was still written.");
    }

    Ok(())
}

fn wait_for_exit() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return;
    }

    print!("\nPress any key to close...");
    let _ = stdout.flush();

    if crossterm::terminal::enable_raw_mode().is_ok() {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(_)) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
    } else {
        let mut line = String::new();
        let _ = stdin.read_line(&mut line);
    }
    println!();
}

#[tokio::main]
async fn main() {
    crypto::init_clock();
    let result = run().await;
    if let Err(error) = &result {
        eprintln!("ERROR: {error:#}");
    }
    wait_for_exit();
    if result.is_err() {
        std::process::exit(1);
    }
}
