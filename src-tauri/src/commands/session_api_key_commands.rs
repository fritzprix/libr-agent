use crate::server::pairing::{
    approve_pairing, get_pending_pairing, list_issued_keys, reject_pending_pairing,
    revoke_issued_key, IssuedApiKeySummary, PendingPairingInfo,
};

#[tauri::command]
pub fn get_pending_session_api_pairing() -> Option<PendingPairingInfo> {
    get_pending_pairing()
}

#[tauri::command]
pub fn approve_session_api_pairing(code: String) -> Result<IssuedApiKeySummary, String> {
    approve_pairing(&code)
}

#[tauri::command]
pub fn reject_session_api_pairing() -> Result<(), String> {
    reject_pending_pairing()
}

#[tauri::command]
pub fn list_session_api_keys() -> Vec<IssuedApiKeySummary> {
    list_issued_keys()
}

#[tauri::command]
pub fn revoke_session_api_key(id: String) -> Result<(), String> {
    revoke_issued_key(&id)
}
