# 📧 Gmail Sender Extractor

A fast and efficient Rust tool to scan all your Gmail messages — across all folders and labels — and generate a **unique list of senders** with message counts.

Built with the official [Google Gmail API](https://developers.google.com/gmail/api) and the [`google-gmail1`](https://crates.io/crates/google-gmail1) crate, this tool uses asynchronous Rust and parallel requests for speed and reliability.

## Features

- Reads **all emails** (including subfolders and labels)  
- Extracts unique **sender addresses** from the “From” header  
- Uses **parallel requests** for high performance  
- Generates a clean CSV report (`gmail_senders_report.csv`)  
- OAuth2 authentication with secure token reuse  

## Requirements

1. **Rust & Cargo**  
   Install from [rustup.rs](https://rustup.rs).

2. **Google Cloud Project**  
   - Go to [Google Cloud Console](https://console.cloud.google.com/apis/dashboard).  
   - Create a new project (or use an existing one).  
   - Enable the **Gmail API**.  
   - Create **OAuth 2.0 Client ID** credentials of type “Desktop App”.  
   - Download the `credentials.json` file and place it in the project root.

3. **Dependencies**

   The project uses:

   ```toml
   google-gmail1 = "6.0.0"
   tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
   futures = "0.3"
   regex = "1"
   csv = "1"
   anyhow = "1"
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"
   ```

Install automatically via Cargo.

## Installation

Clone and build the project:

```bash
git clone https://github.com/yourusername/gmail_senders.git
cd gmail_senders
cargo build --release
```

## Usage

Run the tool:

```bash
cargo run --release
```

First Run

* A browser window will open asking you to log in and authorize Gmail access.
* After authorization, the app saves a token in token.json for future runs.

Output

Once completed, the tool generates gmail_senders_report.csv file


Example:

```bash
Sender	                MessageCount
alice@example.com       42
bob@company.org     	17
newsletter@service.com 	 9
```

## Performance Configuration

You can adjust performance parameters in main.rs:

```rust
const MAX_CONCURRENT_REQUESTS: usize = 10;  // number of parallel API calls
const DELAY_MS_BETWEEN_BATCHES: u64 = 100;  // delay between request batches (ms)
```

:NOTE:
⚠️ Gmail enforces API rate limits.
A concurrency of 10–15 with a small delay (50–150 ms) works well.

## How It Works

1. Authenticates using OAuth2 (credentials.json + token.json)
2. Fetches all Gmail message IDs (across all folders)
3. Runs parallel API calls to get metadata for each message
4. Extracts email addresses from “From” headers via regex
5. Aggregates and writes unique sender counts to CSV

## Files

File	                    Description
src/main.rs	                Main application logic
credentials.json	        OAuth2 credentials from Google Cloud
token.json	                Saved access token for repeat runs
gmail_senders_report.csv	Generated output with sender statistics

## Privacy & Security

- The tool accesses metadata only (no email bodies).
- Your OAuth token (token.json) stays local.
- Nothing is uploaded or shared externally.

## Example Run

In order to execute the component, please do the following:

```bash
$ cargo run --release

Fetching message IDs...
Total messages: 5234
Processed 5200/5234
Unique senders: 812
Report saved as gmail_senders_report.csv
```

## Future Enhancements

- Group results by Gmail labels (Inbox, Sent, Promotions, etc.)
- Filter senders by domain or message count threshold
- Export results as JSON or Markdown

## License

Apache 2.0 © 2025 Fernando López
See LICENSE for details.
