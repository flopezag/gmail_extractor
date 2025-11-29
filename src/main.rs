use anyhow::Result;
use csv::Writer;
use futures::{stream, StreamExt};
use google_gmail1::{
    api::MessagePartHeader, hyper_rustls, hyper_util, Gmail,
    hyper_util::rt::TokioExecutor,
    yup_oauth2 as oauth2,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

const MAX_CONCURRENT_REQUESTS: usize = 10; // parallelism level
const DELAY_MS_BETWEEN_BATCHES: u64 = 100; // small delay to avoid rate-limit

#[tokio::main]
async fn main() -> Result<()> {
    // === Authenticate ===
    let secret = oauth2::read_application_secret("credentials.json").await?;
    let auth = oauth2::InstalledFlowAuthenticator::builder(
        secret,
        oauth2::InstalledFlowReturnMethod::Interactive,
    )
    .persist_tokens_to_disk("token.json")
    .build()
    .await?;

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_or_http()
        .enable_http1()
        .build();

    let client = hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(https);
    let hub = Gmail::new(client, auth);

    println!("Fetching message IDs...");

    // === Fetch all message IDs ===
    let mut message_ids = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut call = hub.users().messages_list("me");
        if let Some(ref token) = page_token {
            call = call.page_token(token);
        }

        let (_, resp) = call.max_results(500).doit().await?;
        if let Some(messages) = resp.messages {
            message_ids.extend(messages.into_iter().filter_map(|m| m.id));
        }

        if let Some(next) = resp.next_page_token {
            page_token = Some(next);
        } else {
            break;
        }
    }

    println!("Total messages: {}", message_ids.len());

    // === Shared state ===
    let re = Regex::new(r"[\w\.-]+@[\w\.-]+").unwrap();
    let counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));

    // === Process in parallel ===
    stream::iter(message_ids.into_iter())
        .chunks(MAX_CONCURRENT_REQUESTS)
        .for_each_concurrent(None, |batch| {
            let hub = &hub;
            let re = &re;
            let counts = Arc::clone(&counts);

            async move {
                let futures = batch.into_iter().map(|msg_id| {
                    let shared_counts = counts.clone();
                    async move {
                    let resp = hub
                        .users()
                        .messages_get("me", &msg_id)
                        .format("metadata")
                        .add_metadata_headers("From")
                        .doit()
                        .await;

                    if let Ok((_, msg)) = resp {
                        if let Some(payload) = msg.payload {
                            if let Some(headers) = payload.headers {
                                for MessagePartHeader { name, value } in headers {
                                    if let Some(n) = name {
                                        if n.to_lowercase() == "from" {
                                            if let Some(val) = value {
                                                if let Some(mat) = re.find(&val) {
                                                    let email = mat.as_str().to_lowercase();
                                                    let mut lock = shared_counts.lock().unwrap();
                                                    *lock.entry(email).or_insert(0) += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    }
                });

                futures::future::join_all(futures).await;
                sleep(Duration::from_millis(DELAY_MS_BETWEEN_BATCHES)).await;
            }
        })
        .await;

    // === Save results ===
    let counts = Arc::try_unwrap(counts)
        .unwrap()
        .into_inner()
        .unwrap();

    println!("Unique senders: {}", counts.len());

    let mut wtr = Writer::from_path("gmail_senders_report.csv")?;
    wtr.write_record(&["Sender", "MessageCount"])?;
    for (sender, count) in counts.iter() {
        wtr.write_record(&[sender, &count.to_string()])?;
    }
    wtr.flush()?;

    println!("Report saved as gmail_senders_report.csv");
    Ok(())
}
