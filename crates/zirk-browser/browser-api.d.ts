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

export type TraceEventKind = "alloc" | "fieldWrite" | "call" | "return" | "throw" | "output";

/** Opt-in, read-only execution trace for `run`. Bounds can only be tightened. */
export interface TraceOptions {
  events?: number;
  bytes?: number;
  valueBytes?: number;
  kinds?: TraceEventKind[];
}

export interface Request {
  apiVersion: 1;
  source: string;
  virtualName: string;
  limits?: Limits;
  trace?: TraceOptions;
}

export interface TraceEvent {
  seq: number;
  kind: TraceEventKind;
  object?: number;
  class?: string;
  field?: string;
  ty?: string;
  value?: string;
  valueTruncated?: boolean;
  callee?: string;
  receiver?: number;
  /** For `output` markers: index into `Response.output`. */
  outputIndex?: number;
}

export interface Trace {
  events: TraceEvent[];
  truncated: boolean;
  limits: { events: number; bytes: number; valueBytes: number };
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
  /** Present only when the request enabled tracing. */
  trace?: Trace;
}

export interface BrowserApi {
  initialize(): void;
  check(request: string): string;
  run(request: string): string;
  /**
   * Runs bounded browser HTTP requests through an explicitly supplied Fetch
   * function. `allowedOrigins` is a JSON array of origin strings. The byte
   * limits apply to request and streamed response bodies. The fetch authority
   * is scoped to this invocation; `run` remains synchronous and hermetic.
   */
  runWithFetch(
    request: string,
    fetchFunction: Function,
    allowedOrigins: string,
    maxRequestBytes: number,
    maxResponseBytes: number,
    abortSignal: AbortSignal | null,
  ): Promise<string>;
  format(request: string): string;
}
