## 2025-05-24 - Zip Slip in Skill Import

**Vulnerability:** Zip Slip vulnerability in `skill_service.rs` allowed arbitrary file write via malicious ZIP archives during skill import. The `zip::ZipArchive::extract` method (v0.6) does not sanitize paths by default.
**Learning:** Libraries like `zip` (before recent versions or specific APIs) often default to unsafe behavior for convenience. Always verify if extraction methods sanitize paths.
**Prevention:** Use `zip::ZipFile::enclosed_name()` to validate paths before extraction. Added `extract_zip_secure` helper in `utils/fs.rs` for safe extraction.

## 2025-05-24 - Symlink Traversal in SecurityValidator

**Vulnerability:** Logic flaw in `SecurityValidator::validate_path` where `!canonical.starts_with(base) && !absolute.starts_with(base)` allowed symlinks pointing outside the workspace to bypass validation because `absolute` (constructed from base) always started with base.
**Learning:** Redundant "double checks" can inadvertently create bypasses if the logic is `AND` instead of `OR`, or if one condition is trivially true. Always verify the canonical path for existing files.
**Prevention:** Fixed the condition to strictly check `canonical_path.starts_with(base)`. Added a regression test case for symlink traversal.
