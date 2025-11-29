use anyhow::Result;
use csv::Writer;
use futures::{stream, StreamExt};
use google_gmail1::{
    api::MessagePartHeader,
    Gmail,
};
use hyper_rustls;
use yup_oauth2 as oauth2;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

const BATCH_SIZE: usize = 100;
const MAX_PARALLEL_BATCHES: usize = 5;      // Safe for Gmail API
const DELAY_MS_BETWEEN_BATCHES: u64 = 80;   // Avoids rate-limit

#[tokio::main]
async fn main() -> Result<()> {
    // === Automatic OAuth, refresh tokens saved to disk ===
    let secret = oauth2::read_application_secret("credentials.json").await?;
    let auth = oauth2::InstalledFlowAuthenticator::builder(
        secret,
        oauth2::InstalledFlowReturnMethod::Interactive,
    )
    .persist_tokens_to_disk("token.json")
    .build()
    .await?;

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots().unwrap()
        .https_or_http()
        .enable_http1()
        .build();

    let client = hyper::Client::builder().build(https);

    let hub = Gmail::new(client, auth);

    println!("📥 Fetching message IDs (all folders)…");

    // === Get all message IDs ===
    let mut message_ids = Vec::new();
    let mut page_token: Option<String> = None;

    let pb_ids = ProgressBar::new_spinner();
    pb_ids.enable_steady_tick(Duration::from_millis(100));
    pb_ids.set_style(
        ProgressStyle::with_template("{spinner} {msg}")?.tick_strings(&[
            "⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"
        ]),
    );
    pb_ids.set_message("Fetching…");

    loop {
        let mut call = hub.users().messages_list("me");
        if let Some(ref token) = page_token {
            call = call.page_token(token);
        }

        let (_, resp) = call.max_results(500).doit().await?;

        if let Some(messages) = resp.messages {
            message_ids.extend(messages.into_iter().filter_map(|m| m.id));
            pb_ids.set_message(format!("Loaded {} IDs…", message_ids.len()));
        }

        if let Some(next) = resp.next_page_token {
            page_token = Some(next);
        } else {
            break;
        }
    }

    pb_ids.finish_with_message(format!("✔ Found {} messages.", message_ids.len()));

    // === Shared state ===
    let counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let re = Regex::new(r"[\w\.-]+@[\w\.-]+").unwrap();

    // === Sender extraction progress bar ===
    let pb = ProgressBar::new(message_ids.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} messages"
        )?
        .progress_chars("##-"),
    );

    // === Split message IDs into batches ===
    let batches: Vec<Vec<String>> = message_ids
        .chunks(BATCH_SIZE)
        .map(|chunk| chunk.iter().cloned().collect())
        .collect();

    println!("🚀 Processing in {} batches ({} msgs each)…",
        batches.len(), BATCH_SIZE);

    stream::iter(batches)
        .for_each_concurrent(MAX_PARALLEL_BATCHES, |batch| {
            let hub = &hub;
            let re = &re;
            let counts = Arc::clone(&counts);
            let pb = pb.clone();

            async move {
                // === Fetch messages individually ===
                for msg_id in &batch {
                    match hub
                        .users()
                        .messages_get("me", msg_id)
                        .format("metadata")
                        .add_metadata_headers("From")
                        .doit()
                        .await
                    {
                        Ok((_, message)) => {
                            if let Some(payload) = message.payload {
                                if let Some(headers) = payload.headers {
                                    for MessagePartHeader { name, value } in headers {
                                        if name.as_deref().unwrap_or("").eq_ignore_ascii_case("from") {
                                            if let Some(val) = value {
                                                if let Some(mat) = re.find(&val) {
                                                    let mut lock = counts.lock().unwrap();
                                                    *lock.entry(mat.as_str().to_lowercase())
                                                        .or_insert(0) += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠ Failed to fetch message {}: {:?}", msg_id, e);
                        }
                    }
                }

                pb.inc(batch.len() as u64);
                sleep(Duration::from_millis(DELAY_MS_BETWEEN_BATCHES)).await;
            }
        })
        .await;

    pb.finish_with_message("✔ Completed all batches.");

    // === Save CSV ===
    let counts = Arc::try_unwrap(counts)
        .unwrap()
        .into_inner()
        .unwrap();

    let mut wtr = Writer::from_path("gmail_senders_report.csv")?;
    wtr.write_record(&["Sender", "MessageCount"])?;

    for (email, count) in counts {
        wtr.write_record(&[email, count.to_string()])?;
    }

    wtr.flush()?;

    println!("📁 Saved gmail_senders_report.csv");
    Ok(())
}
