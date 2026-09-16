/** Versioned JSON contract exposed by the worker-safe WASM facade. */
export type FailureKind = "request" | "runtime" | "capability" | "limit" | "internal" | "version";

export interface Limits {
  sourceBytes: number;
  diagnostics: number;
  instructions: number;
  callDepth: number;
  heapBytes: number;
  outputBytes: number;
  outputEvents: number;
  stackFrames: number;
}

export interface Request {
  apiVersion: 1;
  source: string;
  virtualName: string;
  limits?: Limits;
}

export interface Response {
  apiVersion: 1;
  operation: string;
  compilerVersion: string;
  compilerRevision: string;
  limits: Limits;
  diagnostics: Array<{ code: string; severity: string; message: string; stage: string; file: string | null; start: number | null; end: number | null }>;
  diagnosticsTruncated: boolean;
  output: Array<{ text: string; truncated: boolean }>;
  completion: "completed" | null;
  failure: { kind: FailureKind; code: string; message: string; limit: string | null } | null;
  formattedSource?: string;
}

export interface BrowserApi {
  initialize(): void;
  check(request: string): string;
  run(request: string): string;
  format(request: string): string;
}
