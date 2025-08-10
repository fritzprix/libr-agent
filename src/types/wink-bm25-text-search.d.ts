declare module 'wink-bm25-text-search' {
  interface BM25Config {
    fldWeights?: { [key: string]: number };
    ovlpNormFactor?: number;
    k1?: number;
    b?: number;
  }

  interface BM25Document {
    id: string;
    text: string;
  }

  interface BM25Result {
    0: string; // id
    1: number; // score
  }

  interface BM25Instance {
    defineConfig(config: BM25Config): void;
    addDoc(doc: BM25Document): void;
    consolidate(): void;
    search(query: string): BM25Result[];
  }

  function BM25(): BM25Instance;
  export = BM25;
}
