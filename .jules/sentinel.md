## 2025-05-24 - Zip Slip in Skill Import

**Vulnerability:** Zip Slip vulnerability in `skill_service.rs` allowed arbitrary file write via malicious ZIP archives during skill import. The `zip::ZipArchive::extract` method (v0.6) does not sanitize paths by default.
**Learning:** Libraries like `zip` (before recent versions or specific APIs) often default to unsafe behavior for convenience. Always verify if extraction methods sanitize paths.
**Prevention:** Use `zip::ZipFile::enclosed_name()` to validate paths before extraction. Added `extract_zip_secure` helper in `utils/fs.rs` for safe extraction.
