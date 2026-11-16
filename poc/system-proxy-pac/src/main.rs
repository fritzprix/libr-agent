mod discover;
mod proxy_types;

#[cfg(windows)]
mod windows_sysproxy;

#[cfg(target_os = "macos")]
mod macos_sysproxy;

#[cfg(not(any(windows, target_os = "macos")))]
mod fallback_env;

use clap::Parser;
use proxy_types::{ResolvedProxy, SystemProxyDiscovery};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(
    name = "system-proxy-pac",
    about = "PoC: discover OS proxy (PAC) settings and compare direct vs proxied reqwest GET"
)]
struct Cli {
    /// Target URL to resolve proxy for and fetch (GET).
    url: String,

    #[arg(long, default_value_t = 30)]
    timeout_secs: u64,
}

#[tokio::main]
async fn main() {
    if let Err(code) = run().await {
        eprintln!("error: {code}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let timeout = Duration::from_secs(cli.timeout_secs);

    println!("=== System proxy discovery ===");
    let discovery = discover::discover()?;
    print_discovery(&discovery);

    println!();
    println!("=== Proxy resolved for URL ===");
    println!("URL: {}", cli.url);
    let resolved = discover::resolve(&cli.url)?;
    println!("Result: {}", resolved.display_label());

    println!();
    println!("=== Request A: direct reqwest (no proxy applied) ===");
    let direct_result = fetch_direct(&cli.url, timeout).await;
    print_fetch_result(&direct_result);

    println!();
    println!("=== Request B: reqwest via resolved proxy ===");
    let proxied_result = match &resolved {
        ResolvedProxy::Direct => Err(FetchOutcome {
            elapsed: Duration::ZERO,
            detail: "Skipped: resolved proxy is DIRECT".into(),
        }),
        ResolvedProxy::Proxies(_) => {
            let proxy_url = resolved
                .first_reqwest_proxy_url()
                .ok_or_else(|| "Resolved proxy list was empty".to_string())?;
            fetch_via_proxy(&cli.url, &proxy_url, timeout).await
        }
    };
    print_fetch_result(&proxied_result);

    Ok(())
}

fn print_discovery(d: &SystemProxyDiscovery) {
    match &d.auto_detect {
        Some(v) => println!("auto_detect: {v}"),
        None => println!("auto_detect: (not available on this platform)"),
    }
    print_opt("pac_url", &d.pac_url);
    print_opt("static_proxy", &d.static_proxy);
    print_opt("bypass_list", &d.bypass_list);
    for note in &d.notes {
        println!("note: {note}");
    }
}

fn print_opt(label: &str, value: &Option<String>) {
    match value {
        Some(v) => println!("{label}: {v}"),
        None => println!("{label}: (none)"),
    }
}

struct FetchOutcome {
    elapsed: Duration,
    detail: String,
}

async fn fetch_direct(url: &str, timeout: Duration) -> Result<FetchOutcome, FetchOutcome> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| FetchOutcome {
            elapsed: Duration::ZERO,
            detail: format!("client build failed: {e}"),
        })?;

    run_get(&client, url).await
}

async fn fetch_via_proxy(url: &str, proxy_url: &str, timeout: Duration) -> Result<FetchOutcome, FetchOutcome> {
    println!("Using proxy: {proxy_url}");
    let proxy = reqwest::Proxy::all(proxy_url).map_err(|e| FetchOutcome {
        elapsed: Duration::ZERO,
        detail: format!("invalid proxy URL: {e}"),
    })?;

    // Do not call `.no_proxy()` here: in reqwest it clears every proxy on the builder,
    // including the one added with `.proxy()`, so Request B would still go direct.
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .proxy(proxy)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| FetchOutcome {
            elapsed: Duration::ZERO,
            detail: format!("client build failed: {e}"),
        })?;

    run_get(&client, url).await
}

async fn run_get(client: &reqwest::Client, url: &str) -> Result<FetchOutcome, FetchOutcome> {
    let start = Instant::now();
    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let _ = resp.bytes().await;
            Ok(FetchOutcome {
                elapsed: start.elapsed(),
                detail: format!("HTTP {status}"),
            })
        }
        Err(err) => {
            let kind = if err.is_timeout() {
                "timeout"
            } else if err.is_connect() {
                "connect"
            } else if err.is_request() {
                "request"
            } else {
                "other"
            };
            Err(FetchOutcome {
                elapsed: start.elapsed(),
                detail: format!("error ({kind}): {err}"),
            })
        }
    }
}

fn print_fetch_result(result: &Result<FetchOutcome, FetchOutcome>) {
    match result {
        Ok(ok) => println!(
            "status: success — {} (elapsed {:.2}s)",
            ok.detail,
            ok.elapsed.as_secs_f64()
        ),
        Err(err) => println!(
            "status: failed — {} (elapsed {:.2}s)",
            err.detail,
            err.elapsed.as_secs_f64()
        ),
    }
}
