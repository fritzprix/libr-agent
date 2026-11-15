use std::collections::HashMap;
use std::sync::RwLock;

static HOOK_CONTEXT: std::sync::OnceLock<RwLock<HashMap<String, String>>> =
    std::sync::OnceLock::new();

fn store() -> &'static RwLock<HashMap<String, String>> {
    HOOK_CONTEXT.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn set_hook_context(session_id: &str, text: String) {
    if let Ok(mut map) = store().write() {
        map.insert(session_id.to_string(), text);
    }
}

pub fn get_hook_context(session_id: &str) -> Option<String> {
    store()
        .read()
        .ok()
        .and_then(|map| map.get(session_id).cloned())
}

pub fn clear_hook_context(session_id: &str) {
    if let Ok(mut map) = store().write() {
        map.remove(session_id);
    }
}

pub fn append_hook_context(session_id: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    if let Ok(mut map) = store().write() {
        match map.get_mut(session_id) {
            Some(existing) => {
                if !existing.is_empty() {
                    existing.push_str("\n\n");
                }
                existing.push_str(text);
            }
            None => {
                map.insert(session_id.to_string(), text.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_get_hook_context() {
        let sid = "test-session-hook-context-1";
        clear_hook_context(sid);
        append_hook_context(sid, "first");
        append_hook_context(sid, "second");
        assert_eq!(get_hook_context(sid).as_deref(), Some("first\n\nsecond"));
        set_hook_context(sid, "replaced".to_string());
        assert_eq!(get_hook_context(sid).as_deref(), Some("replaced"));
        clear_hook_context(sid);
        assert!(get_hook_context(sid).is_none());
    }
}
