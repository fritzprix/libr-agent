# macOS Build Issues Analysis

## Current State

### GitHub Actions Configuration

**Release Workflow** (`.github/workflows/release.yml`):
- ✅ Includes `macos-latest` in build matrix
- ✅ Uses proper `tauri-action@v0` for building
- ✅ No macOS-specific dependencies needed (unlike Linux)
- ❌ **Missing: Code signing configuration**
- ❌ **Missing: Notarization configuration**

**CI Workflow** (`.github/workflows/ci.yml`):
- ✅ Tests on `macos-latest`
- ✅ No special dependencies required for macOS
- ✅ Build artifacts uploaded

### Tauri Configuration Issues

**`tauri.conf.json` Analysis:**

1. **Missing macOS Bundle Configuration:**
```json
{
  "bundle": {
    "active": true,
    "targets": "all",  // ✅ Includes macOS
    "icon": [...],
    "linux": { /* Linux config */ },
    // ❌ Missing: "macOS": {} section
  }
}
```

2. **No Code Signing Configuration:**
   - No signing identity specified
   - No entitlements file
   - No Apple Developer Team ID

3. **No Notarization Setup:**
   - macOS 10.15+ requires notarization for distribution
   - GitHub Actions has no Apple credentials configured

## Identified Problems

### 1. 🔴 **Code Signing Not Configured**

**Impact:** 
- Users will see "App is damaged and can't be opened" error
- Gatekeeper will block the app from running
- Users must manually bypass security (right-click → Open)

**Why it happens:**
- macOS requires all apps to be signed (since macOS 10.15+)
- Unsigned apps are immediately blocked by Gatekeeper
- CI builds produce unsigned binaries

**Solution Required:**
```json
// tauri.conf.json
{
  "bundle": {
    "macOS": {
      "signingIdentity": null,  // For development
      "entitlements": null,
      "exceptionDomain": null,
      "frameworks": [],
      "minimumSystemVersion": "10.15"
    }
  }
}
```

### 2. 🔴 **Notarization Not Configured**

**Impact:**
- Even if signed, macOS will show warning for non-notarized apps
- Users must manually allow in System Preferences → Security & Privacy
- Professional apps require notarization

**Why it happens:**
- Notarization requires Apple Developer Program membership ($99/year)
- Requires uploading app to Apple for scanning
- CI needs Apple ID credentials stored as secrets

**Solution Required:**
```yaml
# .github/workflows/release.yml
env:
  APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
  APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
  APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
  APPLE_ID: ${{ secrets.APPLE_ID }}
  APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
  APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
```

### 3. 🟡 **Missing Entitlements File**

**Impact:**
- App may not have necessary permissions
- Certain macOS features may not work (network, filesystem access)
- Security features might block functionality

**Why it happens:**
- No `entitlements.plist` file defined
- Default entitlements may be too restrictive

**Solution Required:**
Create `src-tauri/entitlements.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.cs.allow-jit</key>
  <true/>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
  <true/>
  <key>com.apple.security.cs.allow-dyld-environment-variables</key>
  <true/>
  <key>com.apple.security.network.client</key>
  <true/>
  <key>com.apple.security.network.server</key>
  <true/>
  <key>com.apple.security.files.user-selected.read-write</key>
  <true/>
</dict>
</plist>
```

### 4. 🟡 **No Minimum macOS Version Specified**

**Impact:**
- Unclear which macOS versions are supported
- Potential runtime errors on older systems

**Current:** Not specified
**Recommended:** 10.15 (Catalina) minimum

### 5. 🟡 **Bundle Format Not Optimized**

**Current Output:**
- `.app` bundle
- `.app.tar.gz` for distribution

**Issues:**
- `.tar.gz` is not user-friendly for Mac users
- Should use `.dmg` for better UX

## Recommended Solutions

### Option 1: Ad-Hoc Signing (Quick Fix - Development Only)

**Pros:**
- Free
- No Apple Developer account needed
- Works for local testing

**Cons:**
- Still shows security warnings
- Not suitable for distribution
- Users must manually allow

**Implementation:**
```json
// tauri.conf.json
{
  "bundle": {
    "macOS": {
      "signingIdentity": null  // Uses ad-hoc signing
    }
  }
}
```

### Option 2: Self-Signed Certificate (Better)

**Pros:**
- Free
- Better than ad-hoc
- Can be automated in CI

**Cons:**
- Still requires user manual approval
- Not notarized

**Implementation:**
1. Create self-signed certificate in Keychain
2. Export certificate
3. Add to GitHub Secrets
4. Configure GitHub Actions

### Option 3: Full Apple Developer Signing (Best)

**Pros:**
- ✅ Proper code signing
- ✅ Notarization
- ✅ No security warnings
- ✅ Professional distribution

**Cons:**
- ❌ Costs $99/year (Apple Developer Program)
- ❌ More complex setup

**Implementation:**
1. Join Apple Developer Program
2. Create signing certificates
3. Configure notarization credentials
4. Update GitHub Actions workflow
5. Update tauri.conf.json

## Immediate Action Items

### Priority 1: Make App Runnable (Free Solution)

1. **Add macOS bundle configuration:**
```json
// tauri.conf.json
{
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "10.15",
      "signingIdentity": null
    }
  }
}
```

2. **Update release notes with manual approval instructions:**
```markdown
### macOS Installation

1. Download `LibrAgent.app.tar.gz`
2. Extract the archive
3. Move `LibrAgent.app` to Applications folder
4. **First run:** Right-click → Open (bypass Gatekeeper)
5. Click "Open" in the security dialog
```

3. **Add warning to README:**
```markdown
> ⚠️ **macOS Users:** The app is not code-signed. You'll need to manually 
> allow it in System Preferences → Security & Privacy after first launch.
```

### Priority 2: Improve User Experience

1. **Generate DMG instead of tar.gz:**
   - Configure Tauri to create DMG
   - Better UX for Mac users

2. **Add entitlements file** for proper permissions

3. **Document workaround steps** clearly

### Priority 3: Proper Distribution (Requires Apple Developer Account)

1. Obtain Apple Developer account
2. Set up code signing certificates
3. Configure notarization
4. Update CI/CD pipeline
5. Test on multiple macOS versions

## Testing Checklist

When fixing macOS builds, test:

- [ ] App launches on macOS 10.15 (Catalina)
- [ ] App launches on macOS 11 (Big Sur)
- [ ] App launches on macOS 12+ (Monterey+)
- [ ] Security dialog appears and is bypassable
- [ ] All features work (MCP servers, file access, etc.)
- [ ] Network requests succeed
- [ ] File system access works
- [ ] Tray icon appears correctly

## Additional Resources

- [Tauri macOS Bundle Configuration](https://tauri.app/v1/guides/building/macos)
- [Apple Code Signing Guide](https://developer.apple.com/support/code-signing/)
- [macOS Notarization](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)

## Summary

**Current Problem:** macOS builds are not code-signed, causing Gatekeeper to block them.

**Quick Fix:** Document manual workaround for users (right-click → Open)

**Proper Fix:** Requires Apple Developer account ($99/year) for code signing and notarization.

**Recommendation:** Start with Option 1 (ad-hoc signing + user instructions), then upgrade to Option 3 when ready to invest in proper distribution.
