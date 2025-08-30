// BM25 related type definitions
export type BM25SearchResult = [string, number];

export interface LocalBM25Instance {
  defineConfig: (config: object) => void;
  addDoc: (doc: { id: string; text: string }) => void;
  consolidate: () => void;
  search: (query: string) => BM25SearchResult[];
}
