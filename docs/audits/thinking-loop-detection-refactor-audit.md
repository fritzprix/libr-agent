# 기술 분석 보고서: Thinking/Text Loop Detection Refactor

---

## 1. 개요 (Executive Summary)

- **분석 대상**: LLM 스트리밍 내 Thinking/Text Loop 감지 로직 분리 및 사용자 설정 가능화
- **핵심 목표**: Thinking과 Text 스트림의 루프 감지 임계값을 분리하고, Thinking 측을 사용자 설정으로 열어보내 아키텍처적 유연성 확보
- **최종 판정**: ★★★★☆ — 설계 방향은 우수하나, 설정 전파 경로에 검증 허점이 존재

---

## 2. 주요 작업 분석 (Detailed Analysis)

### 작업 A: 루프 감지 임계값 분리 (shared → per-domain)

**변경 사항:**

```typescript
// Before: 단일 공유 config
export const REPEATED_LOOP_CONFIG: RepeatedTailDetectorConfig = {
  minPatternLength: 64,
  minRepetitions: 3,
  tailChars: 1024,
};
// thinking/text 모두 REPEATED_LOOP_CONFIG 사용

// After: 독립적인 config 두 개
export const REPEATED_THINKING_CONFIG = {
  minPatternLength: 256,
  minRepetitions: 4,
  tailChars: 2048,
};
export const REPEATED_TEXT_CONFIG = {
  minPatternLength: 64,
  minRepetitions: 3,
  tailChars: 1024,
};
```

**설계적 의의:**

- **ISP(Interface Segregation Principle)** 준수: Thinking 스트림은 긴 추론 패턴을 포함하므로 더 높은 `minPatternLength`(64→256)와 `tailChars`(1024→2048)가 필요
- Text 스트림은 짧은 명령 반복이 많으므로 기존 임계값 유지 — **False Positive 방지**
- 함수 시그니처에 `config?` 파라미터 추가 → **테스트 용이성 향상**

**효과:**

- Thinking 루프 감지 민감도 대폭 ↓ (false positive 감소)
- Text 루프 감지는 기존 동작 그대로 유지 (하위 호환성)

### 작업 B: 사용자 설정 가능화 (Settings UI + Runtime injection)

**변경 사항:**

- `AdvancedSettings` 인터페이스에 `thinkingLoopMinPatternLength`, `thinkingLoopMinRepetitions` 추가
- `AdvancedRuntimeControlsSection.tsx`에 두 개의 `NumberSettingField` UI 컨트롤 추가
- `useExecuteCompletion.ts`에서 runtime에 settings 값을 읽어 config 객체 생성

**설계적 의의:**

- **사용자 경험**: LLM 추론 특성에 따라 민감도를 조절할 수 있는 flexibility 제공
- **안전한 범위**: Pattern Length 32-1024, Repetitions 2-10 — 과도한 변경 방지

**효과:**

- 사용자가 루프 감지를 미세 조정 가능
- 디버깅/튜닝 시 invaluable

### 작업 C: Generated 파일 lint 포맷 수정

**변경 사항:**

- `builtin-services.ts`: `typeof ... [number]` → `(typeof ...)[number]` (TS lint parentheses)
- `execution-mode.ts`: 다중 줄 배열 → 단일 줄 (Prettier)

**설계적 의의:**

- ESLint `@stylistic/parentheses` 및 Prettier 규칙 준수 — **코드베이스 일관성 유지**

---

## 3. 아키텍처 및 품질 평가

| 항목                      | 평가 (1-5) | 상세 피드백                                                                                                                                                                                                                                                                                                                                                                        |
| :------------------------ | :--------: | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **모듈화 (Modularity)**   |     4      | Domain별 config 분리는 excellent. 다만 `useExecuteCompletion.ts`의 inline config 생성이 `repeatedTailDetector.ts`의 상수를 일부 재정의하는 구조로, **설정이 두 군데**(상수 + default UI 값)에 분산됨.                                                                                                                                                                              |
| **인터페이스 설계 (ISP)** |     5      | `detectRepeatedThinkingLoop(text, config?)` — config 파라미터 선택적, 디폴드 값 명확. Text 도메인도 같은 함수 시그니처로 지원. 깔끔한 분리.                                                                                                                                                                                                                                        |
| **중복 제거 (DRY)**       |     3      | ⚠️ `useExecuteCompletion.ts`에서 `thinkingConfig` 객체를 **매 스트림 체크마다 새로 생성** (매 32 chunk마다). config 값이 불변이므로 **외부에서 한 번만 생성**하거나 **memoize**하는 것이 효율적. 또한 default 값 256/4가 `REPEATED_THINKING_CONFIG`와 `NumberSettingField`의 fallback에 **세 군데**에 분산.                                                                        |
| **성능/비용 (Caching)**   |     3      | 위 DRY 문제와 연관: 매 스트림 체크마다 config 객체 할당. `REPEATED_THINKING_TAIL_CHARS` 상수는 import하지만 `minPatternLength`/`minRepetitions`는 하드코딩된 디폴트 값으로 재작성.                                                                                                                                                                                                 |
| **안정성 (Reliability)**  |     3      | ⚠️ **Critical**: `rust-settings-service.ts`의 `isPartialAdvancedSettings` whitelist에는 새 필드가 추가되었으나, **서버 DTO → Settings 매핑 로직**에 기본값이 명시적으로 설정되지 않으면 `undefined`로 저장될 수 있음. `settings-service.ts`의 `DEFAULT_SETTING.advanced`에는 기본값이 있으므로 클라이언트 디폴트는 안전하나, **서버-클라이언트 설정 동기화**가 확실한지 검증 필요. |
| **테스트 품질**           |     4      | Thinking 테스트의 `REPEATED_THINKING_MIN_REPETITIONS` 상수 참조는 good. 다만 `toBeGreaterThan` 테스트는 **구현 상세**를 테스트할 뿐, 실제 감지기 동작을 검증하지는 않음. 새로운 4-repetition 기준의 end-to-end 시나리오는 기존 테스트가 커버.                                                                                                                                      |

---

## 4. 리스크 및 향후 제언

### 잠재적 부작용 (Side Effects)

1. **설정 동기화 리스크 (P0) — RESOLVED**
   - `rust-settings-service.ts`의 `advanced` 매핑을 확인함:
     ```typescript
     advanced: { ...DEFAULT_SETTING.advanced, ...getTypedValue('advancedSettings', {}) }
     ```
   - `DEFAULT_SETTING.advanced`에 `thinkingLoopMinPatternLength: 256`, `thinkingLoopMinRepetitions: 4`가 이미 있으므로, 기존 DB 레코드에도 **spread로 기본값이 자동 적용됨**
   - **결론: P0 아님.** 기존 설정이 없는 새 DB도 디폴트로 안전.

2. **UI ↔ Code Default 불일치 (P1)**
   - `NumberSettingField`의 `fallback: 256` / `fallback: 4`
   - `useExecuteCompletion.ts`의 `?? 256` / `?? 4`
   - `REPEATED_THINKING_CONFIG`의 `256` / `4`
   - 이 세 군데가 동시에 변경되면 **한 군데만 놓치는 가능성** 존재

3. **Text Loop 설정 부재 (P2)**
   - Thinking은 설정 UI로 열었지만 Text는 여전히 하드코딩됨
   - 향후 일관성을 위해 Text도 설정 가능하게 만드는 것이 좋을 것

4. **성능: 매 체크마다 config 객체 할당 (P2)**
   - `repeatedThinkingCheckCounter % REPEATED_TAIL_CHECK_INTERVAL === 0` 조건에서 객체가 계속 새로 생성됨
   - 실제 영향은 미미하지만, 100% clean code라면 `useMemo` 또는 모듈 레벨 `useCallback`으로 처리

### 기술 부채 (Remaining Tasks)

- [ ] `rust-settings-service.ts`의 DTO-Settings 매핑에 새 필드 기본값 추가 확인
- [ ] config 디폴트 값을 한 곳으로 통합 (예: `DEFAULT_THINKING_CONFIG` 상수 + UI에서 `?? DEFAULT_THINKING_CONFIG`)
- [ ] Text loop 감지도 설정 UI로 열지 여부 의사결정
- [ ] `useExecuteCompletion.ts`에서 config 객체 생성을 메모이제이션

### 다음 액션 (Recommended Next Steps)

1. **검증**: `pnpm refactor:validate` 실행하여 빌드/타입 체크 통과 확인
2. **서버 매핑 확인**: `rust-settings-service.ts` lines 80-120 확인 — 새 필드에 대한 `getTypedValue()` 호출이 있는지
3. **E2E 테스트**: 실제 LLM 스트리밍에서 설정 변경이 감지 임계값에 반영되는지 확인

---

## 5. 결론 (Conclusion)

이 리팩토링은 **올바른 방향**으로 진행되었습니다. Thinking과 Text 루프 감지를 분리한 것은 LLM 추론 특성에 대한 정확한 인식에서 비롯된 설계 결정이며, 사용자 설정 가능성을 열어둔 것은 좋은 UX 선택입니다.

다만, **디폴트 값이 세 군데에 분산**되어 있고 **서버 설정 매핑의 완전성이 검증되지 않았으며**, **스트림 체크마다 config 객체를 새로 할당**하는 것은 개선 여지가 있습니다. P0 설정 동기화 문제를 확인한 후 merge하면 큰 문제없이 운영할 수 있습니다.

**Overall: ★★★★☆ (4/5)** — Architecture is solid, minor hygiene fixes needed.
