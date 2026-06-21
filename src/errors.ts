export type RemoteErrorContext = Readonly<Record<string, string>>;

export class RemoteOpsError extends Error {
  readonly code: string;
  readonly retryable: boolean;
  readonly correlationId: string;
  readonly context: RemoteErrorContext;

  constructor(
    message: string,
    code: string,
    retryable: boolean,
    correlationId: string,
    context: RemoteErrorContext = {},
  ) {
    super(message);
    this.name = "RemoteOpsError";
    this.code = code;
    this.retryable = retryable;
    this.correlationId = correlationId;
    this.context = context;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isContext(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every((entry) => typeof entry === "string");
}

export function normalizeRemoteError(error: unknown): RemoteOpsError {
  if (error instanceof RemoteOpsError) return error;

  if (error instanceof Error) {
    return new RemoteOpsError(error.message, "client.error", false, "");
  }

  if (
    isRecord(error) &&
    typeof error.message === "string" &&
    typeof error.code === "string" &&
    typeof error.retryable === "boolean" &&
    typeof error.correlation_id === "string" &&
    isContext(error.context)
  ) {
    return new RemoteOpsError(
      error.message,
      error.code,
      error.retryable,
      error.correlation_id,
      Object.freeze({ ...error.context }),
    );
  }

  return new RemoteOpsError("An unknown client error occurred.", "client.unknown", false, "");
}
