import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { backend } from "../src/services/backend";

const invokeMock = vi.mocked(invoke);

describe("backend command contract", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({});
  });

  it("passes preview input through the preview_archive command", async () => {
    await backend.preview({
      rootPath: "D:\\repo\\运营支撑门户",
      product: "25_user_extension",
      relativePaths: ["a.groovy"]
    });
    expect(invokeMock).toHaveBeenCalledWith("preview_archive", {
      input: {
        rootPath: "D:\\repo\\运营支撑门户",
        product: "25_user_extension",
        relativePaths: ["a.groovy"]
      }
    });
  });

  it("passes confirmation flags through confirm_archive", async () => {
    await backend.confirm({ planId: "p1", confirmed: true, tenantAckPaths: ["a.groovy"] });
    expect(invokeMock).toHaveBeenCalledWith("confirm_archive", {
      input: { planId: "p1", confirmed: true, tenantAckPaths: ["a.groovy"] }
    });
  });

  it("uses root_path and limit for history", async () => {
    await backend.history("D:\\repo", 20);
    expect(invokeMock).toHaveBeenCalledWith("list_archive_runs", {
      rootPath: "D:\\repo",
      limit: 20
    });
  });
});