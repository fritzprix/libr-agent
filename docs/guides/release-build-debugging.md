# Release 빌드 디버깅 가이드

## 📋 문제 요약

**증상**: Release 빌드에서 "Could not connect to localhost: Connection refused" 오류 발생

**근본 원인**: `cargo build --release`와 `tauri build`의 차이

## 🔍 원인 분석

### 1. 두 가지 빌드 방식의 차이

#### ❌ 잘못된 방법: `cargo build --release`
```bash
cd src-tauri
cargo build --release
```
**문제점**:
- Rust 백엔드 바이너리만 빌드됨
- 프론트엔드 `dist` 폴더가 번들에 포함되지 않음
- 바이너리가 실행될 때 프론트엔드 파일을 찾을 수 없어 실패

#### ✅ 올바른 방법: `tauri build`
```bash
pnpm tauri build
# 또는
pnpm build && cd src-tauri && cargo tauri build
```
**작동 과정**:
1. `pnpm build` → 프론트엔드를 `dist/` 폴더에 빌드
2. `tauri build` → Rust 빌드 + 프론트엔드를 바이너리에 번들링
3. 최종 결과물: `.deb`, `.rpm`, `.AppImage` 등 완전한 패키지

### 2. tauri.conf.json 설정

```json
{
  "build": {
    "beforeDevCommand": "pnpm dev",           // dev 모드: localhost:1420
    "devUrl": "http://localhost:1420",        // ← dev 서버 URL
    "beforeBuildCommand": "pnpm build",       // release: dist 폴더 빌드
    "frontendDist": "../dist"                 // ← release에서 사용할 정적 파일 경로
  }
}
```

**동작 방식**:
- **Dev 모드** (`pnpm tauri dev`): 
  - `devUrl`을 사용 → Vite dev 서버(localhost:1420)에 연결
  - Hot reload 지원
  
- **Release 모드** (`pnpm tauri build`):
  - `frontendDist` 폴더의 정적 파일을 번들에 포함
  - localhost 연결 불필요
  - 완전히 독립적인 앱

### 3. 바이너리 위치

```bash
# cargo build --release (백엔드만)
src-tauri/target/release/synaptic-flow  # ❌ 프론트엔드 없음

# tauri build (완전한 앱)
src-tauri/target/release/bundle/
├── deb/SynapticFlow_0.1.1_amd64.deb                      # ✅ Debian 패키지
├── rpm/SynapticFlow-0.1.1-1.x86_64.rpm                   # ✅ RPM 패키지
└── appimage/SynapticFlow_0.1.1_amd64.AppImage            # ✅ AppImage
```

## 🚀 올바른 빌드 & 실행 방법

### 개발 중 (Debug)
```bash
# 터미널 1: 프론트엔드 dev 서버
pnpm dev

# 터미널 2: Tauri dev 모드
pnpm tauri dev
```

### 릴리스 빌드 (Production)
```bash
# 1. 의존성 설치
pnpm install

# 2. 완전한 릴리스 빌드
pnpm tauri build

# 3. 결과물 실행 (Linux 예시)
./src-tauri/target/release/bundle/appimage/SynapticFlow_*.AppImage
```

### 릴리스 빌드 테스트
```bash
# 로그와 함께 실행
RUST_LOG=debug RUST_BACKTRACE=1 \
  ./src-tauri/target/release/bundle/appimage/SynapticFlow_*.AppImage
```

## 🔧 디버깅 팁

### 1. 로그 수집
```bash
# 상세 로그와 함께 실행
RUST_LOG=trace RUST_BACKTRACE=full \
  ./SynapticFlow_*.AppImage > app.log 2>&1

# 로그 확인
tail -f app.log
```

### 2. 프론트엔드 번들 확인
```bash
# dist 폴더가 제대로 생성되었는지 확인
ls -la dist/

# index.html과 assets 폴더 확인
ls -la dist/assets/
```

### 3. 번들에 포함된 파일 확인 (AppImage)
```bash
# AppImage 내용 추출
./SynapticFlow_*.AppImage --appimage-extract

# 번들된 파일 확인
ls -la squashfs-root/usr/bin/
```

### 4. WebView 디버거 활성화

`src-tauri/tauri.conf.json`에 추가:
```json
{
  "app": {
    "windows": [{
      "devtools": true  // ← 릴리스 빌드에서도 DevTools 활성화
    }]
  }
}
```

## ⚠️ 주의사항

### 1. CI/CD에서 빌드
GitHub Actions나 다른 CI/CD에서는 반드시 `tauri build`를 사용:

```yaml
# ❌ 잘못된 예
- run: cargo build --release

# ✅ 올바른 예
- run: pnpm install
- run: pnpm tauri build
```

### 2. 환경 변수
- **Dev**: `.env` 파일에서 로드 (`#[cfg(debug_assertions)]`)
- **Release**: 시스템 환경 변수 사용

### 3. 윈도우 서브시스템 설정
`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```
- Release에서 콘솔 창을 숨김 (Windows만 해당)
- Linux에서는 영향 없음

## 📊 트러블슈팅 체크리스트

- [ ] `pnpm install` 실행했는가?
- [ ] `pnpm build`로 `dist/` 폴더 생성했는가?
- [ ] `tauri build` (not `cargo build`)를 사용했는가?
- [ ] `tauri.conf.json`의 `frontendDist` 경로가 올바른가?
- [ ] 번들 파일(`bundle/` 폴더)을 실행하고 있는가?
- [ ] RUST_LOG 환경 변수로 로그를 확인했는가?

## 🎯 결론

**핵심**: Tauri 앱은 프론트엔드와 백엔드가 결합된 하이브리드 앱입니다.
- Development: 별도의 dev 서버 사용
- Production: 정적 파일을 바이너리에 번들링

**항상 `pnpm tauri build`를 사용하여 완전한 앱을 빌드하세요!**
