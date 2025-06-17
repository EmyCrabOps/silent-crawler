# Silent Crawler

A fast, concurrent web crawler written in Rust, designed to efficiently discover URLs, directories, and subdomains of a target website — with optional respect for `robots.txt`.

---

## 🚀 Features

- **Concurrent Crawling**: Processes multiple URLs simultaneously with configurable concurrency.
  
- **Robots.txt Support**: Optionally respects `robots.txt` rules.
  
- **Customizable Parameters**: Control depth, delays, timeouts, user agent, etc.

  
- **Output Options**: Print results to console or save as JSON.
- **URL Normalization**: Handles relative URLs, strips fragments, and normalizes paths.
- **Domain Filtering**: Stays within the target domain and subdomains.
- **Comprehensive Results**: Collects full lists of URLs, directories, and subdomains.

---

## 📦 Installation

Ensure you have Rust installed. Clone the repo and build the project:

```bash
git clone https://github.com/EmyCrabOps/silent-crawler.git
cd silent-crawler
cargo build --release
```

The binary will be available in:  
`target/release/silent-crawler`

---

## ⚙️ Usage

Run the crawler with:

```bash
./silent-crawler <URL> [OPTIONS]
```

---

## 🧰 Options

```text
-d, --depth <DEPTH>           Maximum recursion depth (default: 3)

-w, --wait <SECONDS>          Delay between requests in seconds (default: 0.5)

-t, --timeout <SECONDS>       Request timeout in seconds (default: 10)

-u, --user-agent <STRING>     Custom User-Agent string (default: Mozilla/5.0...)

-o, --output <PATH>           Output file path for results in JSON format

--ignore-robots               Ignore robots.txt (default: false)

-c, --concurrency <NUMBER>    Max concurrent requests (default: 10)
```

---

## 📌 Examples

#### Basic crawl:

```bash
./silent-crawler https://example.com
```

#### Custom depth, delay, and output file:

```bash
./silent-crawler https://example.com -d 5 -w 1.0 -o results.json
```

#### Ignore robots.txt and increase concurrency:

```bash
./silent-crawler https://example.com --ignore-robots -c 20
```

---

## 📤 Output

If `--output` is used, the JSON result will look like:

```json
{
  "urls": [
    "https://example.com/",
    "https://example.com/about/",
    ...
  ],
  "directories": [
    "/about/",
    "/blog/",
    ...
  ],
  "subdomains": [
    "sub1.example.com",
    "sub2.example.com",
    ...
  ]
}
```

---

## 🧱 Dependencies

Add these to your `Cargo.toml`:

```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
reqwest = { version = "0.12", features = ["gzip"] }
scraper = "0.20"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.40", features = ["full"] }
url = "2.5"
rand = "0.8"
futures = "0.3"
```

---

## 📒 Notes

- Adds a random jitter (0–0.5s) to delays to avoid predictable patterns.
- Only processes HTML content (`text/html`) for link extraction.
- Skips `javascript:`, `mailto:`, `tel:`, and fragment (`#`) URLs.
- Ensures full URL normalization and directory consistency.

---


## 👤 Author

**Silent Crawler** by [EmyCrabOps](https://github.com/EmyCrabOps)
