import { describe, expect, it } from "vitest";

import { RemoteOpsError } from "./errors";

describe("RemoteOpsError", () => {
  it("stringifies to the message alone, without a class-name prefix", () => {
    const error = new RemoteOpsError(
      "Received disconnect from 10.0.0.1 port 22:2: Too many authentication failures",
      "remote.operation_failed",
      true,
      "corr-1",
    );

    expect(String(error)).toBe(
      "Received disconnect from 10.0.0.1 port 22:2: Too many authentication failures",
    );
    expect(`${error}`).toBe(
      "Received disconnect from 10.0.0.1 port 22:2: Too many authentication failures",
    );
  });
});
