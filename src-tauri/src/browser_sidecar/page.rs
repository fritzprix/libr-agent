use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::emulation::{
    ClearDeviceMetricsOverrideParams, SetDeviceMetricsOverrideParams,
};
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, Viewport};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use log::warn;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::contracts::{HistoryNavigationStatus, PageClassification, PageState};

const HISTORY_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(4);
const HISTORY_NAVIGATION_POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Cap how long chromiumoxide may wait for full page load (networkIdle-prone sites).
pub(crate) const PAGE_LOAD_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum accepted PNG payload size after capture.
pub const MAX_SCREENSHOT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum CSS content pixels allowed for full-page capture (matches chromiumoxide scale=1).
pub const MAX_FULL_PAGE_PIXELS: f64 = 64_000_000.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NavigationSnapshot {
    url: String,
    title: String,
    ready_state: String,
    history_length: u64,
    history_state: Option<String>,
    body_text_snippet: String,
}

#[derive(Debug, Clone, Copy)]
enum HistoryDirection {
    Back,
    Forward,
}

pub(crate) async fn snapshot_page_state(page: &Page) -> Result<PageState, String> {
    let snapshot = snapshot_navigation_state(page).await?;
    Ok(page_state_from_snapshot(snapshot))
}

/// Navigate and bound the load wait so ad-heavy / never-idle pages cannot hang forever.
///
/// On timeout the page is kept open and a partial snapshot is returned so callers can
/// still extract whatever DOM has already rendered.
pub(crate) async fn goto_with_load_timeout(page: &Page, url: &str) -> Result<PageState, String> {
    let timed = tokio::time::timeout(PAGE_LOAD_TIMEOUT, page.goto(url)).await;
    let timed_out = match timed {
        Ok(Ok(_)) => false,
        Ok(Err(error)) => {
            return Err(format!("Failed to navigate to '{url}': {error}"));
        }
        Err(_) => {
            warn!(
                "Page load timed out for {url} after {:?}; continuing with partial page state",
                PAGE_LOAD_TIMEOUT
            );
            true
        }
    };

    let mut state = snapshot_page_state(page).await?;
    if timed_out {
        state.navigation_message = Some(format!(
            "Navigation load wait timed out after {}s; page may be partially loaded",
            PAGE_LOAD_TIMEOUT.as_secs()
        ));
    }
    Ok(state)
}

pub(crate) async fn navigate_back(page: &Page) -> Result<PageState, String> {
    perform_history_navigation(page, HistoryDirection::Back).await
}

pub(crate) async fn navigate_forward(page: &Page) -> Result<PageState, String> {
    perform_history_navigation(page, HistoryDirection::Forward).await
}

pub(crate) fn serialize_evaluation_result(
    result: chromiumoxide::js::EvaluationResult,
) -> Result<String, String> {
    serialize_browser_result_value(result.value().cloned())
}

/// Validate full-page CSS content dimensions before capture.
pub fn validate_full_page_dimensions(width: f64, height: f64) -> Result<(), String> {
    if !width.is_finite() || !height.is_finite() {
        return Err("Full-page screenshot dimensions are invalid".to_string());
    }
    let width = width.max(0.0);
    let height = height.max(0.0);
    let pixel_count = width * height;
    if !pixel_count.is_finite() {
        return Err("Full-page screenshot dimensions are invalid".to_string());
    }
    if pixel_count > MAX_FULL_PAGE_PIXELS {
        return Err(format!(
            "Full-page screenshot is too large: {:.0} pixels exceeds the limit of {:.0}",
            pixel_count, MAX_FULL_PAGE_PIXELS
        ));
    }
    Ok(())
}

/// Validate captured PNG payload size.
pub fn validate_screenshot_png_bytes(screenshot: &[u8]) -> Result<(), String> {
    if screenshot.len() > MAX_SCREENSHOT_BYTES {
        return Err(format!(
            "Screenshot is too large: {} bytes exceeds the limit of {} bytes",
            screenshot.len(),
            MAX_SCREENSHOT_BYTES
        ));
    }
    Ok(())
}

pub(crate) async fn capture_screenshot(page: &Page, full_page: bool) -> Result<Vec<u8>, String> {
    if full_page {
        capture_full_page_screenshot(page).await
    } else {
        capture_viewport_screenshot(page).await
    }
}

async fn capture_viewport_screenshot(page: &Page) -> Result<Vec<u8>, String> {
    let screenshot = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(false)
                .build(),
        )
        .await
        .map_err(|e| format!("Failed to capture browser screenshot: {e}"))?;
    validate_screenshot_png_bytes(&screenshot)?;
    Ok(screenshot)
}

/// Full-page capture with guaranteed device-metrics cleanup.
///
/// chromiumoxide's `full_page(true)` sets `SetDeviceMetricsOverride` and only clears it on the
/// success path. We own that lifecycle here so a mid-capture failure cannot leave the page stuck
/// at an oversized viewport for later browser tools.
async fn capture_full_page_screenshot(page: &Page) -> Result<Vec<u8>, String> {
    let metrics = page
        .layout_metrics()
        .await
        .map_err(|e| format!("Failed to measure page for full-page screenshot: {e}"))?;
    let size = metrics.css_content_size;
    validate_full_page_dimensions(size.width, size.height)?;
    let width = size.width.max(0.0);
    let height = size.height.max(0.0);

    page.execute(SetDeviceMetricsOverrideParams::new(
        width as i64,
        height as i64,
        1.0,
        false,
    ))
    .await
    .map_err(|e| format!("Failed to prepare full-page screenshot viewport: {e}"))?;

    let capture_result = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(false)
                .clip(Viewport {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                    scale: 1.0,
                })
                .build(),
        )
        .await;

    if let Err(clear_error) = page
        .execute(ClearDeviceMetricsOverrideParams::default())
        .await
    {
        warn!("Failed to clear device metrics override after full-page screenshot: {clear_error}");
    }

    let screenshot =
        capture_result.map_err(|e| format!("Failed to capture browser screenshot: {e}"))?;
    validate_screenshot_png_bytes(&screenshot)?;
    Ok(screenshot)
}

async fn snapshot_navigation_state(page: &Page) -> Result<NavigationSnapshot, String> {
    evaluate_json(
        page,
        r#"(function() {
            let historyState = null;
            try {
                historyState = history.state === undefined ? null : JSON.stringify(history.state);
            } catch (_error) {
                historyState = "__LIBRAGENT_UNSERIALIZABLE_HISTORY_STATE__";
            }

            const bodyText = document.body
                ? ((document.body.innerText || document.body.textContent || '').slice(0, 512))
                : '';

            return {
                url: window.location.href,
                title: document.title || '',
                readyState: document.readyState || '',
                historyLength: Math.max(history.length || 0, 0),
                historyState,
                bodyTextSnippet: bodyText
            };
        })()"#,
    )
    .await
}

fn page_state_from_snapshot(snapshot: NavigationSnapshot) -> PageState {
    let classification =
        classify_browser_page(&snapshot.url, &snapshot.title, &snapshot.body_text_snippet);
    PageState {
        url: snapshot.url,
        title: if snapshot.title.is_empty() {
            None
        } else {
            Some(snapshot.title)
        },
        classification: Some(classification),
        navigation_status: None,
        navigation_message: None,
    }
}

async fn evaluate_json<T: DeserializeOwned>(page: &Page, script: &str) -> Result<T, String> {
    page.evaluate(script)
        .await
        .map_err(|e| format!("Failed to evaluate browser state script: {e}"))?
        .into_value()
        .map_err(|e| format!("Failed to decode browser state value: {e}"))
}

async fn perform_history_navigation(
    page: &Page,
    direction: HistoryDirection,
) -> Result<PageState, String> {
    let mut before = match snapshot_navigation_state(page).await {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            warn!(
                "{} navigation pre-snapshot failed; continuing without baseline: {}",
                direction.label(),
                error
            );
            None
        }
    };
    let trigger_script = match direction {
        HistoryDirection::Back => "history.back(); 'Navigated back'",
        HistoryDirection::Forward => "history.forward(); 'Navigated forward'",
    };
    page.evaluate(trigger_script)
        .await
        .map_err(|e| format!("Failed to trigger {} navigation: {e}", direction.label()))?;

    let deadline = tokio::time::Instant::now() + HISTORY_NAVIGATION_TIMEOUT;
    let mut latest = before.clone();
    let mut last_snapshot_error: Option<String> = None;
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }

        tokio::time::sleep(HISTORY_NAVIGATION_POLL_INTERVAL).await;
        let current = match snapshot_navigation_state(page).await {
            Ok(current) => {
                last_snapshot_error = None;
                current
            }
            Err(error) => {
                last_snapshot_error = Some(error);
                continue;
            }
        };
        if let Some(previous) = before.as_ref() {
            if navigation_snapshot_changed(previous, &current) {
                let mut state = page_state_from_snapshot(current);
                state.navigation_status = Some(HistoryNavigationStatus::Navigated);
                return Ok(state);
            }
        } else {
            before = Some(current.clone());
        }
        latest = Some(current);
    }

    let Some(latest) = latest else {
        return Err(match last_snapshot_error {
            Some(error) => format!(
                "Failed to observe page state after {} navigation: {}",
                direction.label(),
                error
            ),
            None => format!(
                "Failed to observe page state after {} navigation",
                direction.label()
            ),
        });
    };

    let classification =
        classify_browser_page(&latest.url, &latest.title, &latest.body_text_snippet);
    let mut state = page_state_from_snapshot(latest);
    match classification {
        PageClassification::BlockedInterstitial => {
            state.navigation_status = Some(HistoryNavigationStatus::BlockedInterstitial);
            state.navigation_message = Some(format!(
        "{} navigation did not complete because the current page appears to be a CAPTCHA or blocking interstitial",
        direction.label()
      ));
        }
        PageClassification::Normal => {
            state.navigation_status = Some(HistoryNavigationStatus::NoHistoryEntry);
            state.navigation_message = Some(format!(
                "{} navigation produced no observable page change",
                direction.label()
            ));
        }
    }

    Ok(state)
}

fn navigation_snapshot_changed(before: &NavigationSnapshot, after: &NavigationSnapshot) -> bool {
    before.url != after.url
        || before.title != after.title
        || before.history_length != after.history_length
        || before.history_state != after.history_state
}

pub fn classify_browser_page(
    url: &str,
    title: &str,
    body_text_snippet: &str,
) -> PageClassification {
    let url_lower = url.to_ascii_lowercase();
    let title_lower = title.to_ascii_lowercase();
    let body_lower = body_text_snippet.to_ascii_lowercase();
    let combined = format!("{title_lower}\n{body_lower}");

    let is_google_sorry = url::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            let host = parsed.host_str()?.to_ascii_lowercase();
            let path = parsed.path().to_ascii_lowercase();
            Some(
                (host == "google.com"
                    || host.ends_with(".google.com")
                    || host.starts_with("google."))
                    && path.starts_with("/sorry"),
            )
        })
        .unwrap_or_else(|| url_lower.contains("google.com/sorry") || url_lower.contains("/sorry/"));

    if is_google_sorry
        || url_lower.contains("captcha")
        || combined.contains("captcha")
        || combined.contains("recaptcha")
        || combined.contains("unusual traffic")
        || combined.contains("verify you are human")
        || combined.contains("verify you're human")
        || combined.contains("checking your browser before accessing")
        || combined.contains("cf challenge")
        || combined.contains("__cf_chl")
    {
        return PageClassification::BlockedInterstitial;
    }

    PageClassification::Normal
}

impl HistoryDirection {
    fn label(self) -> &'static str {
        match self {
            HistoryDirection::Back => "Back",
            HistoryDirection::Forward => "Forward",
        }
    }
}

pub fn serialize_browser_result_value(value: Option<Value>) -> Result<String, String> {
    Ok(match value {
        None => "undefined".to_string(),
        Some(Value::String(text)) => text,
        Some(Value::Null) => "null".to_string(),
        Some(other) => serde_json::to_string(&other)
            .map_err(|e| format!("Failed to serialize JavaScript result: {e}"))?,
    })
}
