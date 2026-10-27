import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/analysis/api";
import type { SolveReport } from "../lib/analysis/types";
import { shownReport, useWorkspace } from "./workspaceStore";
import { useContingencies } from "./contingencyStore";
import type { ContingencyDetail } from "../lib/analysis/types";

const report = (tag: string) => ({ status: tag }) as unknown as SolveReport;
const detail = (rank: number, element: string) => ({ row: { rank, element } }) as unknown as ContingencyDetail;

describe("the shown result", () => {
  beforeEach(() => {
    useWorkspace.setState({ caseVersion: 1, solveReport: report("latest"), shown: null });
    useContingencies.setState({ detail: null, open: null, onDiagram: false });
  });
  afterEach(() => vi.restoreAllMocks());

  it("is the latest solve until a named result loads", async () => {
    let resolve!: (r: SolveReport) => void;
    const fetch = vi.spyOn(api, "resultReport").mockReturnValue(new Promise((r) => (resolve = r)));
    const pending = useWorkspace.getState().showResult({ name: "contingency 3", label: "Outage #3", element: "line:1-2-1" });
    expect(fetch).toHaveBeenCalledWith("contingency 3");
    expect(shownReport(useWorkspace.getState())?.status).toBe("latest");
    resolve(report("outage"));
    await pending;
    expect(shownReport(useWorkspace.getState())?.status).toBe("outage");
    useWorkspace.getState().showLatest();
    expect(shownReport(useWorkspace.getState())?.status).toBe("latest");
  });

  it("goes back to the latest solve when a new solve arrives", async () => {
    vi.spyOn(api, "resultReport").mockResolvedValue(report("trip"));
    await useWorkspace.getState().showResult({ name: "trip line:1-2-1", label: "Tripped", element: "line:1-2-1" });
    useWorkspace.getState().setSolveReport(report("again"));
    expect(useWorkspace.getState().shown).toBeNull();
    expect(shownReport(useWorkspace.getState())?.status).toBe("again");
  });

  it("drops an answer that arrives after the solve moved on", async () => {
    let resolve!: (r: SolveReport) => void;
    vi.spyOn(api, "resultReport").mockReturnValue(new Promise((r) => (resolve = r)));
    const pending = useWorkspace.getState().showResult({ name: "contingency 1", label: "Outage #1", element: "line:1-2-1" });
    useWorkspace.getState().setSolveReport(report("newer"));
    resolve(report("old outage"));
    await pending;
    expect(shownReport(useWorkspace.getState())?.status).toBe("newer");
  });

  it("follows the outage drawn on the diagram", async () => {
    const fetch = vi.spyOn(api, "resultReport").mockResolvedValue(report("outage"));
    useContingencies.setState({ open: 2, detail: detail(2, "line:4-5-1") });
    useContingencies.getState().setOnDiagram(true);
    expect(fetch).toHaveBeenCalledWith("contingency 2");
    expect(useWorkspace.getState().shown).toMatchObject({ name: "contingency 2", label: "Outage #2", element: "line:4-5-1" });
    useContingencies.getState().setOnDiagram(false);
    expect(useWorkspace.getState().shown).toBeNull();
  });

  it("names a trip after its element", () => {
    const fetch = vi.spyOn(api, "resultReport").mockResolvedValue(report("trip"));
    useContingencies.setState({ open: null, detail: detail(1, "gen:3-1") });
    useContingencies.getState().setOnDiagram(true);
    expect(fetch).toHaveBeenCalledWith("trip gen:3-1");
    useContingencies.getState().clear();
    expect(useWorkspace.getState().shown).toBeNull();
  });
});
