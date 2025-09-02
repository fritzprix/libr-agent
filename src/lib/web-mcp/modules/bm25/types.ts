// BM25 related type definitions
export type BM25SearchResult = [string, number];

export interface OkapiBM25Result {
  document: string;
  score: number;
}

// okapibm25 라이브러리의 실제 반환 타입
export interface BMDocument {
  document: string;
  score: number;
}

export type OkapiBM25Function = {
  (
    documents: string[],
    query: string[],
    options?: { k1?: number; b?: number },
  ): number[];
  (
    documents: string[],
    query: string[],
    options: { k1?: number; b?: number },
    sortFunction: (a: BMDocument, b: BMDocument) => number,
  ): BMDocument[];
};
