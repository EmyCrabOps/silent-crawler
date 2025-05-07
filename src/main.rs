use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};
use rand::Rng;
use reqwest::{Client, header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE, ACCEPT_ENCODING, CONNECTION, UPGRADE_INSECURE_REQUESTS}};
use scraper::{Html, Selector};
use serde::Serialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use url::{Url, ParseError};

#[derive(Parser, Debug)]
#[clap(author = "Silent Crawler", version, about = "A fast web crawler written in Rust")]
struct Args {
    /// Base URL to crawl
    #[clap(index = 1)]
    url: String,

    /// Maximum recursion depth
    #[clap(short, long, default_value = "3")]
    depth: usize,

    /// Delay between requests in seconds
    #[clap(short, long, default_value = "0.5")]
    wait: f64,

    /// Request timeout in seconds
    #[clap(short, long, default_value = "10")]
    timeout: u64,

    /// Custom User-Agent string
    #[clap(short, long)]
    user_agent: Option<String>,

    /// Output file path for results (JSON format)
    #[clap(short, long)]
    output: Option<PathBuf>,

    /// Ignore robots.txt restrictions
    #[clap(long)]
    ignore_robots: bool,

    /// Maximum number of concurrent requests
    #[clap(short = 'c', long, default_value = "100")]
    concurrency: usize,
}

#[derive(Serialize, Debug)]
struct Results {
    urls: Vec<String>,
    directories: Vec<String>,
    subdomains: Vec<String>,
}

struct SilentCrawler {
    base_url: String,
    base_domain: String,
    max_depth: usize,
    delay: f64,
    client: Client,
    visited_urls: Arc<Mutex<HashSet<String>>>,
    directories: Arc<Mutex<HashSet<String>>>,
    subdomains: Arc<Mutex<HashSet<String>>>,
    disallowed_paths: Arc<Mutex<HashSet<String>>>,
    respect_robots: bool,
    concurrency: usize,
}

impl SilentCrawler {
    async fn new(
        base_url: &str,
        max_depth: usize,
        delay: f64,
        timeout: u64,
        user_agent: Option<&str>,
        respect_robots: bool,
        concurrency: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Validate and normalize the base URL
        let mut url = base_url.to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            url = format!("http://{}", url);
        }
        
        // Parse the base URL to extract domain
        let parsed_url = Url::parse(&url)?;
        let base_domain = parsed_url.host_str()
            .ok_or("Invalid URL: missing domain")?
            .to_string();
            
        // Create HTTP client with headers and timeout
        let mut headers = HeaderMap::new();
        
        // Define default user agent if none provided
        let ua = user_agent.unwrap_or(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
        );
        
        headers.insert(USER_AGENT, HeaderValue::from_str(ua)?);
        headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
        
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(timeout))
            .build()?;
        
        let crawler = SilentCrawler {
            base_url: url,
            base_domain,
            max_depth,
            delay,
            client,
            visited_urls: Arc::new(Mutex::new(HashSet::new())),
            directories: Arc::new(Mutex::new(HashSet::new())),
            subdomains: Arc::new(Mutex::new(HashSet::new())),
            disallowed_paths: Arc::new(Mutex::new(HashSet::new())),
            respect_robots,
            concurrency,
        };
        
        // Parse robots.txt if required
        if respect_robots {
            crawler.parse_robots_txt().await?;
        }
        
        Ok(crawler)
    }

    async fn parse_robots_txt(&self) -> Result<(), Box<dyn std::error::Error>> {
        let robots_url = format!("{}/robots.txt", self.base_url.trim_end_matches('/'));
        
        match self.client.get(&robots_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(text) = response.text().await {
                        let mut disallowed_paths = self.disallowed_paths.lock().unwrap();
                        
                        for line in text.lines() {
                            let line = line.trim().to_lowercase();
                            if line.starts_with("disallow:") {
                                if let Some(path) = line.split(':').nth(1) {
                                    let path = path.trim();
                                    if !path.is_empty() {
                                        disallowed_paths.insert(path.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // If we can't access robots.txt, continue with empty disallowed paths
            }
        }
        
        Ok(())
    }

    fn is_allowed(&self, url: &str) -> bool {
        if !self.respect_robots {
            return true;
        }
        
        if let Ok(parsed_url) = Url::parse(url) {
            let path = parsed_url.path();
            
            let disallowed_paths = self.disallowed_paths.lock().unwrap();
            for disallowed in &*disallowed_paths {
                if path.starts_with(disallowed) {
                    return false;
                }
            }
        }
        
        true
    }

    fn is_same_domain(&self, url: &str) -> bool {
        if let Ok(parsed_url) = Url::parse(url) {
            if let Some(url_domain) = parsed_url.host_str() {
                // Check for subdomain of base domain
                return url_domain == self.base_domain || url_domain.ends_with(&format!(".{}", self.base_domain));
            }
        }
        false
    }

    fn normalize_url(&self, url: &str, source_url: &str) -> Result<String, ParseError> {
        // Convert relative URL to absolute URL
        let base_url = Url::parse(source_url)?;
        let absolute_url = base_url.join(url)?;
        
        // Remove fragments
        let mut normalized_url = absolute_url.to_string();
        if let Some(pos) = normalized_url.find('#') {
            normalized_url.truncate(pos);
        }
        
        // Ensure URL ends with / if it's a directory-like URL (no file extension)
        let path = absolute_url.path();
        if !path.is_empty() && 
           !path.split('/').last().unwrap_or("").contains('.') && 
           !normalized_url.ends_with('/') {
            normalized_url.push('/');
        }
        
        Ok(normalized_url)
    }

    fn extract_links(&self, html_content: &str, source_url: &str) -> HashSet<String> {
        let mut links = HashSet::new();
        
        // Parse HTML
        let document = Html::parse_document(html_content);
        
        // Select all anchor tags with href attributes
        let selector = Selector::parse("a[href]").unwrap();
        
        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                // Skip unwanted link types
                if href.starts_with("javascript:") || 
                   href.starts_with("mailto:") || 
                   href.starts_with("tel:") || 
                   href.starts_with('#') {
                    continue;
                }
                
                // Normalize the URL
                if let Ok(absolute_url) = self.normalize_url(href, source_url) {
                    // Only include URLs from the same domain
                    if self.is_same_domain(&absolute_url) {
                        links.insert(absolute_url);
                    }
                }
            }
        }
        
        links
    }

    fn extract_subdomain(&self, url: &str) -> Option<String> {
        if let Ok(parsed_url) = Url::parse(url) {
            if let Some(domain) = parsed_url.host_str() {
                // If the domain is the same as the base domain, no subdomain
                if domain == self.base_domain {
                    return None;
                }
                
                // Extract the subdomain part
                if domain.ends_with(&format!(".{}", self.base_domain)) {
                    return Some(domain[..domain.len() - self.base_domain.len() - 1].to_string());
                }
            }
        }
        
        None
    }

    fn extract_directory(&self, url: &str) -> Option<String> {
        if let Ok(parsed_url) = Url::parse(url) {
            let path = parsed_url.path();
            
            // No path or just root
            if path.is_empty() || path == "/" {
                return None;
            }
            
            // Directory-like path (ends with /)
            if path.ends_with('/') {
                return Some(path.to_string());
            }
            
            // File-like path, get the directory part
            if let Some(last_slash_pos) = path.rfind('/') {
                if last_slash_pos > 0 {  // Not just starting with /
                    return Some(path[..=last_slash_pos].to_string());
                }
            }
        }
        
        None
    }

    async fn fetch_url(&self, url: &str) -> Option<String> {
        match self.client.get(url).send().await {
            Ok(response) => {
                // Check for successful response
                if response.status().is_success() {
                    if let Some(content_type) = response.headers().get("content-type") {
                        let content_type = content_type.to_str().unwrap_or("");
                        
                        // Only process HTML content
                        if content_type.contains("text/html") {
                            if let Ok(text) = response.text().await {
                                return Some(text);
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Silently handle any request errors
            }
        }
        
        None
    }

    async fn crawl(&self) -> Results {
        // Start crawling from base URL
        self.crawl_concurrent(&self.base_url, 0).await;
        
        // Prepare and return results
        let visited_urls = self.visited_urls.lock().unwrap();
        let mut urls: Vec<String> = visited_urls.iter().cloned().collect();
        urls.sort();
        
        let directories = self.directories.lock().unwrap();
        let mut dirs: Vec<String> = directories.iter().cloned().collect();
        dirs.sort();
        
        let subdomains = self.subdomains.lock().unwrap();
        let mut subs: Vec<String> = subdomains.iter().cloned().collect();
        subs.sort();
        
        Results {
            urls,
            directories: dirs,
            subdomains: subs,
        }
    }

    async fn crawl_concurrent(&self, start_url: &str, depth: usize) {
        // Don't crawl beyond max depth
        if depth > self.max_depth {
            return;
        }
        
        // Initialize queue with start URL if valid
        let mut queue = FuturesUnordered::new();
        
        // Check if the URL is allowed and not already visited
        {
            let visited = self.visited_urls.lock().unwrap();
            if !visited.contains(start_url) && self.is_allowed(start_url) {
                queue.push(self.process_url(start_url.to_string(), depth));
            }
        }
        
        // Process the queue with bounded concurrency
        while let Some(next_urls) = queue.next().await {
            // Add new discovered URLs to the queue if not at max depth
            if depth < self.max_depth {
                for url in next_urls {
                    let visited = self.visited_urls.lock().unwrap();
                    if !visited.contains(&url) && self.is_allowed(&url) {
                        queue.push(self.process_url(url, depth + 1));
                        
                        // Limit concurrent tasks
                        if queue.len() >= self.concurrency {
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_url(&self, url: String, _depth: usize) -> Vec<String> {
        // Add URL to visited set
        {
            let mut visited = self.visited_urls.lock().unwrap();
            visited.insert(url.clone());
        }
        
        // Extract and store subdomain if present
        if let Some(subdomain) = self.extract_subdomain(&url) {
            let mut subdomains = self.subdomains.lock().unwrap();
            subdomains.insert(subdomain);
        }
        
        // Extract and store directory if present
        if let Some(directory) = self.extract_directory(&url) {
            let mut directories = self.directories.lock().unwrap();
            directories.insert(directory);
        }
        
        // Add a small delay between requests
        let jitter = rand::rng().random_range(0.0..0.5);
        sleep(Duration::from_secs_f64(self.delay + jitter)).await;
        
        // Fetch page content
        if let Some(html_content) = self.fetch_url(&url).await {
            // Extract links from the page
            return self.extract_links(&html_content, &url).into_iter().collect();
        }
        
        Vec::new()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();
    
    println!("Starting silent crawler on {}", args.url);
    println!("Max depth: {}, Delay: {}s, Timeout: {}s, Concurrent requests: {}", 
             args.depth, args.wait, args.timeout, args.concurrency);
    println!("Respecting robots.txt: {}", !args.ignore_robots);
    
    // Initialize crawler
    let crawler = SilentCrawler::new(
        &args.url,
        args.depth,
        args.wait,
        args.timeout,
        args.user_agent.as_deref(),
        !args.ignore_robots,
        args.concurrency,
    ).await?;
    
    // Run the crawler
    let results = crawler.crawl().await;
    
    // Display summary
    println!("\nCrawl Summary:");
    println!("Total URLs discovered: {}", results.urls.len());
    println!("Directories found: {}", results.directories.len());
    println!("Subdomains discovered: {}", results.subdomains.len());
    
    // Output detailed results if requested
    if let Some(output_path) = args.output {
        let json = serde_json::to_string_pretty(&results)?;
        let mut file = File::create(output_path.clone())?;
        file.write_all(json.as_bytes())?;
        println!("\nDetailed results saved to {}", output_path.display());
    } else {
        // Print results to stdout
        println!("\nDiscovered Directories:");
        for directory in &results.directories {
            println!("  {}", directory);
        }
        
        println!("\nDiscovered Subdomains:");
        for subdomain in &results.subdomains {
            println!("  {}", subdomain);
        }
    }
    
    Ok(())
}
