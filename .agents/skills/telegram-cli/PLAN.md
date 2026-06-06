# Telegram CLI Skill — 기획안 (Final)

## 1. 개요

LibrAgent 에이전트가 Telegram을 직접 조작할 수 있도록 하는 스킬.
Telethon (MTProto) 기반의 Python CLI 도구로 구성되며, 메시지 송수신, 채팅 목록 조회, 파일 다운로드, 검색 등 6가지 핵심 동작을 지원한다.

---

## 2. 아키텍처

```
사용자 요청
    │
    ▼
SKILL.md (디스패처)
    ├── Step 1: check_config.py → 설정 상태 확인
    ├── Step 2: setup.py         → 최초 인증 (API ID/Hash → phone → code → 2FA)
    └── Step 3: telegram_cli.py  → 동작 실행 (send/get/list/search/download/info)
```

### 2.1 구성 파일

| 파일              | 경로                                   | 역할                                               |
| ----------------- | -------------------------------------- | -------------------------------------------------- |
| `SKILL.md`        | `.agents/skills/telegram-cli/SKILL.md` | 스킬 정의, 워크플로우, 명령어 가이드               |
| `check_config.py` | `scripts/check_config.py`              | 인증 상태 체크 (0=OK, 1=미설정, 2=깨짐)            |
| `setup.py`        | `scripts/setup.py`                     | 인증 세션 생성 (API ID/Hash → phone → code → save) |
| `telegram_cli.py` | `scripts/telegram_cli.py`              | 실제 동작 수행 (dispatch 패턴)                     |

### 2.2 데이터 저장 위치

- 설정 파일: `~/.libragent/telegram_config.json`
- 세션 파일: `~/.libragent/telegram_session.session` (Telethon이 자동 생성)
- **전역 공유**: 모든 세션에서 동일한 설정/세션 파일 사용

---

## 3. 인증 워크플로우 (Step 2)

### 3.1 전체 흐름

```
[1] API ID/Hash 획득 가이드 → my.telegram.org
[2] 채팅에서 api_id, api_hash, phone 수집
[3] PowerShell hidden prompt → 인증 코드 입력 (inputType=password)
[4] setup.py --action send_code
[5] setup.py --action sign_in --code-env LIBRAGENT_TELEGRAM_CODE
[6] 2FA 활성화 시 → setup.py --action sign_in --password-env LIBRAGENT_TELEGRAM_PASSWORD
[7] Remove-Item Env:LIBRAGENT_TELEGRAM_CODE/PASSWORD (클린업)
[8] 완료 → Step 3으로 진행
```

### 3.2 보안 규칙 (email-integration과 동일 패턴)

| 항목            | 채팅 허용 | Hidden Prompt           |
| --------------- | --------- | ----------------------- |
| API ID          | ✅        | —                       |
| API Hash        | ✅        | —                       |
| Phone           | ✅        | —                       |
| 인증 코드 (SMS) | ❌        | ✅ `inputType=password` |
| 2FA 비밀번호    | ❌        | ✅ `inputType=password` |

**핵심 원칙**:

- 민감값(code, password)은 절대 채팅에서 입력받지 않음
- `runInPersistentPowerShell` + `inputType=password`로 숨김 입력
- 환경변수로 스크립트에 전달, 즉시 삭제
- API Hash를 채팅에 붙여넣어도 — 에코하지 않고 바로 `setup.py` 실행

### 3.3 PowerShell 예시

```powershell
# Step A: 코드 전송 요청
python "<skill-base-dir>/scripts/setup.py" `
  --api-id 12345678 `
  --api-hash "abcdef0123456789..." `
  --phone "+821012345678" `
  --action send_code

# Step B: 숨김 코드 입력
python "<skill-base-dir>/scripts/setup.py" `
  --api-id 12345678 `
  --api-hash "abcdef0123456789..." `
  --phone "+821012345678" `
  --action sign_in `
  --code-env LIBRAGENT_TELEGRAM_CODE

# 클린업
Remove-Item Env:LIBRAGENT_TELEGRAM_CODE -ErrorAction SilentlyContinue
Remove-Item Env:LIBRAGENT_TELEGRAM_PASSWORD -ErrorAction SilentlyContinue
```

---

## 4. 동작별 API (Step 3)

### 4.1 동작 목록

| Action            | 설명             | 주요 파라미터                                    |
| ----------------- | ---------------- | ------------------------------------------------ |
| `send_message`    | 메시지 전송      | `--chat`, `--message`, `--file` (옵션)           |
| `get_messages`    | 최근 메시지 조회 | `--chat`, `--limit` (기본 20), `--offset_offset` |
| `list_chats`      | 채팅 목록        | 없음                                             |
| `search_messages` | 메시지 검색      | `--query`, `--limit`, `--chat` (옵션)            |
| `download_file`   | 파일 다운로드    | `--chat`, `--message_id`, `--dest`               |
| `get_chat_info`   | 채팅 정보        | `--chat`                                         |

### 4.2 채팅 식별자

- `@username` 또는 `username` (유저/채널)
- `-1001234567890` (그룹/채널 ID)
- `123456789` (개인 메시지 ID)
- `me` (저장된 메시지)

---

## 5. 출력 포맷

### 5.1 메시지 목록

```
📨 텔레그램 메시지 (chat: @example_channel)

#  날짜              내용
1  06/01 14:23      오늘 회의는 14시에 시작됩니다.
2  06/01 13:45      [이미지]
3  06/01 12:00      새로운 기능 배포 완료

더 보려면: "다음 20개 보여줘"
```

### 5.2 전송 확인

```
✅ 텔레그램 메시지 발송 완료
  받는 곳: @example_channel
  발송 시각: 2026-06-01 14:30
```

### 5.3 채팅 목록

```
💬 텔레그램 채팅 목록 (총 25개)

#  이름                    유형       마지막 활동
1  ● LibrAgent Dev         채널       10분 전
2    GitHub Notifications  채널       1시간 전
3  ● 프로젝트 A            그룹(42)   3시간 전
4    김철수                개인       어제

● = 읽지 않음
```

---

## 6. 에러 처리 매핑

| 에러                 | 원인                 | 대응                            |
| -------------------- | -------------------- | ------------------------------- |
| Auth required        | 세션 만료/무효       | Step 2 재실행                   |
| FloodWait            | Telegram 레이트 리밋 | 지정한 초 대기 후 재시도        |
| Chat not found       | 잘못된 채팅 식별자   | 사용자 확인 요청                |
| File download failed | 크기 제한/권한       | 파일 크기 확인, 대체 방법       |
| API hash invalid     | 잘못/취소된 해시     | Step 2 재실행 (새로운 자격증명) |
| Phone number invalid | 포맷 오류            | `+82...` 국제 포맷 안내         |

**원칙**: 에러만 보고하지 말고, 다음 구체적인 행동을 제시한다.

---

## 7. 구현 상태

| 파일               | 상태    | 비고                              |
| ------------------ | ------- | --------------------------------- |
| `SKILL.md`         | ✅ 완료 | Step 2 보안 패턴 반영 완료        |
| `check_config.py`  | ✅ 완료 | Exit code 0/1/2 로직              |
| `telegram_cli.py`  | ✅ 완료 | dispatch 패턴, 6개 액션           |
| `setup.py`         | ✅ 완료 | `--action send_code/sign_in` 지원 |
| `requirements.txt` | ✅ 완료 | `telethon` 의존성 정의            |

---

## 8. 향후 개선 방향

1. **그룹/채팅 자동 완성** — `list_chats` 결과를 캐싱하여 `--chat` 파라미터 추천
2. **대용량 파일 다운로드** — `download_file`에 진행률 표시 추가
3. **스태커 알림** — `get_messages`에 읽지 않은 메시지 수 표시
4. **Webhook 대체** — 현재 폴링 기반이지만, 장기적으로 Telegram Bot API 연동도 고려
