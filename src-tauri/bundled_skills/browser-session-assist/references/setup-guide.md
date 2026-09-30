# Saved browser logins — setup cues

## English

1. Open **Settings → System → Saved browser logins**.
2. **Import** a Chrome, Edge, or Brave profile into LibrAgent’s private copy (your everyday browser is unchanged).
3. Set a **default** profile if more than one is imported (agents use the default with `use_profile: true`).
4. For **Google**: if login fails in automation or you see “This browser or app may not be secure”, tap **Open to sign in**, finish login in the LibrAgent Chrome window, then close it.
5. Return to the agent and confirm so it can retry `browser__createSession({ use_profile: true })`.

If createSession says the saved login is **already open**, close the LibrAgent Open-to-sign-in Chrome window first, then retry — that is not the same as needing Open to sign in again.

Firefox is not supported for saved logins.

## 한국어

1. **설정 → 시스템 → 저장된 브라우저 로그인** 으로 이동합니다.
2. **Chrome / Edge / Brave** 프로필을 LibrAgent 전용 복사본으로 **가져오기** 합니다 (평소 쓰는 브라우저는 그대로입니다).
3. 여러 개면 에이전트가 쓸 **기본 프로필**을 지정합니다 (`use_profile: true`).
4. **Google**: 자동화 로그인이 막히거나 “안전한 브라우저가 아님”이 뜨면 **Open to sign in(로그인용으로 열기)** 으로 LibrAgent Chrome 창에서 직접 로그인한 뒤 창을 닫습니다.
5. 채팅에서 완료를 알리면 에이전트가 `use_profile: true` 세션을 다시 엽니다.

세션 생성이 **이미 열려 있다**고 하면 Open to sign in으로 연 LibrAgent Chrome 창을 먼저 닫고 재시도합니다 (미로그인과 혼동하지 마세요).

Firefox는 지원하지 않습니다.

## Agent notes

- Do not ask the user for profile filesystem paths.
- Do not claim you are controlling their everyday Chrome window.
- After import or Open to sign in, always retry `createSession` with explicit prior confirmation still in force for this task.
