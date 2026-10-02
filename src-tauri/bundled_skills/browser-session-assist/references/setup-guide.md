# Saved browser logins — setup cues

## English

1. Open **Settings → System → Saved browser logins**.
2. Tap **Start import…** and complete the guided steps:
   1. **Choose** Chrome / Edge / Brave
   2. **Close** those browsers (required — login files are locked while open)
   3. **Copy** into LibrAgent’s private folder
   4. **Done**
3. Set a **default** profile if more than one is saved (agents use the default with `use_profile: true`).
4. For **Google** (or if a site still asks you to log in): expand **Google still asks you to sign in?** → **Open Chrome to sign in** → finish login → **I’m done signing in**. **Do not leave that window open** while the agent runs.
5. Return to the agent and confirm so it can retry `browser__createSession({ use_profile: true })`.

If createSession says the saved login is **already open**, finish/close that LibrAgent Chrome window first, then retry — that is not the same as needing Google recovery again.

Firefox is not supported for saved logins.

## 한국어

1. **설정 → 시스템 → 저장된 브라우저 로그인** 으로 이동합니다.
2. **가져오기 시작…** 을 누르고 안내 단계를 끝까지 진행합니다:
   1. 브라우저 **선택**
   2. 해당 브라우저 **종료** (열려 있으면 로그인 파일이 잠김)
   3. LibrAgent 전용 폴더로 **복사**
   4. **완료**
3. 여러 개면 에이전트가 쓸 **기본 프로필**을 지정합니다 (`use_profile: true`).
4. **Google**(또는 사이트가 또 로그인을 물으면): **Google 로그인이 또 필요한가요?** 펼치기 → **로그인용 Chrome 열기** → 로그인 → **로그인 마쳤어요**. **창을 연 채로 에이전트를 돌리지 마세요.**
5. 채팅에서 완료를 알리면 에이전트가 `use_profile: true` 세션을 다시 엽니다.

세션 생성이 **이미 열려 있다**고 하면 LibrAgent Chrome 창을 먼저 닫고 재시도합니다 (미로그인과 혼동하지 마세요).

Firefox는 지원하지 않습니다.

## Agent notes

- Do not ask the user for profile filesystem paths.
- Do not claim you are controlling their everyday Chrome window.
- Never tell the user to keep the recovery Chrome window open for agent attach.
- After import wizard or Google recovery + done, always retry `createSession` with explicit prior confirmation still in force for this task.
- Do not pretend import is one click — name the close + copy steps when guiding.
