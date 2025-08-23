# Tauri + transformers.js + ONNX Runtime 기반 임베딩 구현 및 마이그레이션 가이드

## 1. Tauri에서 ONNX Runtime 사용법

- Rust 백엔드에서 ONNX Runtime을 직접 사용하거나, sidecar(외부 바이너리)로 포함해 실행 가능
- `tauri.conf.json`의 `externalBin`에 ONNX Runtime(또는 Python 등) 바이너리 경로 지정
- Rust에서 `tauri_plugin_shell`, JS에서 `@tauri-apps/plugin-shell`로 sidecar 실행
- 복잡한 ML 연산은 Rust/sidecar에서 처리, UI는 JS/TS로 경량화

## 2. transformers.js로 ONNX 모델 실행

- transformers.js는 Hugging Face transformer 모델을 브라우저/Node.js에서 ONNX Runtime(WASM/WebGPU) 기반으로 실행
- `pipeline` API로 임베딩, 분류 등 다양한 태스크 실행 가능
- 예시:
  ```js
  import { pipeline } from '@huggingface/transformers';
  const embedder = await pipeline(
    'feature-extraction',
    'Xenova/all-MiniLM-L6-v2',
  );
  const embeddings = await embedder(['문장1', '문장2']);
  ```

## 3. sentence-transformers/all-MiniLM-L6-v2 임베딩 구현

- 384차원 경량 sentence embedding 모델, semantic search 등에서 널리 사용
- transformers.js로 ONNX 변환 모델을 불러와 JS/TS에서 임베딩 생성 가능
- quantization, WebGPU 등 옵션으로 성능 최적화

## 4. 최신 경량 임베딩 모델 비교

- `all-MiniLM-L6-v2`는 속도-품질 균형이 우수
- 최신(2025) 기준, Voyage-3-large(상용, 고성능), Stella 400M(오픈소스, 경량, 성능 우수) 등이 대안
- Voyage-3-lite, Stella 400M이 비용-성능 균형이 좋음

## 5. Python 임베딩 서비스(Tauri로 마이그레이션)

- Python의 embedding_service.py(예: sentence-transformers 기반)를 Tauri로 옮기는 방법:
  - Rust로 재구현: Rust용 ONNX Runtime 바인딩 활용, 모델 ONNX 변환 필요
  - Python sidecar: tauri-plugin-python 등으로 Python 백엔드(sidecar) 실행, 기존 코드 재사용
  - JS/TS(Frontend): transformers.js로 임베딩 생성, 클라이언트에서 직접 처리
  - Hybrid: Rust orchestration + JS/TS 경량 임베딩 + 복잡 연산은 Python sidecar

## 6. Best Practice & Cross-language Interop

- ONNX Runtime으로 Rust/JS에서 고성능 추론
- 모델은 Hugging Face Optimum 등으로 ONNX 변환
- sidecar는 tauri.conf.json에 등록, 권한 설정 필요
- transformers.js는 경량 임베딩, WebGPU, quantization 지원
- embedding 결과값은 Python/JS간 일치 검증 필요
- Rust async, Tauri command로 sidecar와 효율적 통신

---

### 참고자료

- [Transformers.js 공식문서](https://huggingface.co/docs/transformers.js/en/index)
- [all-MiniLM-L6-v2 모델카드](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)
- [Tauri Sidecar 공식문서](https://v2.tauri.app/develop/sidecar)
- [Python→Rust 마이그레이션 가이드](https://corrode.dev/learn/migration-guides/python-to-rust)
- [Embedding 모델 비교(2025)](https://www.datastax.com/blog/best-embedding-models-information-retrieval-2025)
