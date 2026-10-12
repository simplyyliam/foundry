/**
 * Small typed builders for backend data, so a test only spells out the fields it cares about.
 * Each takes overrides; everything else gets a plain default.
 */
import type {
  AgentReport,
  ChangedFile,
  CommitStatus,
  ManagerDigest,
  Plan,
  PlanRequirement,
  PlanSummary,
  SliceDigest,
  SliceReport,
  SliceView,
  TeamAgent,
  TestCommands,
} from "@/lib/bindings";

export const PROJECT_ID = "p-test";

export function slice(overrides: Partial<SliceView> & Pick<SliceView, "id" | "name">): SliceView {
  return {
    delivers: [],
    dependsOn: [],
    status: "queued",
    decisions: [],
    openQuestions: [],
    startCommit: null,
    workspaceAccess: false,
    layer: 0,
    usedBy: 0,
    ready: true,
    ...overrides,
  };
}

export function requirement(overrides: Partial<PlanRequirement> & Pick<PlanRequirement, "id" | "text">): PlanRequirement {
  return {
    kind: "behavior",
    sliceId: null,
    sourceExcerpt: "",
    verifiedBy: "",
    state: "proposed",
    unsourced: false,
    projectWide: false,
    ...overrides,
  };
}

export function plan(overrides: Partial<Plan> = {}): Plan {
  return { revision: 1, slices: [], requirements: [], deferred: [], openQuestions: [], ...overrides };
}

export function summary(overrides: Partial<PlanSummary> = {}): PlanSummary {
  return { unassigned: [], unconfirmed: [], ready: [], blocked: [], acyclic: true, confirmed: false, ...overrides };
}

export function sliceDigest(overrides: Partial<SliceDigest> & Pick<SliceDigest, "sliceId" | "name">): SliceDigest {
  return {
    status: "queued",
    reportPresent: false,
    reportCount: 0,
    verification: "none",
    skip: null,
    ...overrides,
  };
}

export function digest(overrides: Partial<ManagerDigest> = {}): ManagerDigest {
  return { projectId: PROJECT_ID, generatedAt: 0, agents: [], slices: [], attention: [], ...overrides };
}

export function sliceReport(overrides: Partial<SliceReport> & Pick<SliceReport, "sliceId" | "agent">): SliceReport {
  return { requirements: [], testsRun: [], blockers: [], notes: "", ...overrides };
}

export function agentReport(sliceId: string, agent: string, report: Partial<SliceReport> = {}): AgentReport {
  return {
    path: `.foundry/reports/${sliceId}.${agent}.json`,
    agent,
    report: sliceReport({ sliceId, agent, ...report }),
    writtenAt: "2026-10-01T12:00:00.000Z",
  };
}

export function teamAgent(overrides: Partial<TeamAgent> & Pick<TeamAgent, "id" | "displayName" | "title">): TeamAgent {
  return {
    avatarPath: null,
    reads: [],
    writeGlobs: [],
    protectedGlobs: [],
    sourceHandbookRoleId: null,
    ...overrides,
  };
}

export function testCommands(overrides: Partial<TestCommands> = {}): TestCommands {
  return { commands: [], confirmed: true, ...overrides };
}

export function changedFile(path: string, overrides: Partial<ChangedFile> = {}): ChangedFile {
  return { path, change: "modified", foundryOwned: false, ...overrides };
}

/** `commitStatus` with nothing to commit unless `files` is given (`total` follows `files`). */
export function commitStatus(overrides: Partial<CommitStatus> = {}): CommitStatus {
  const files = overrides.files ?? [];
  return {
    head: "0123456789abcdef",
    branch: "main",
    files,
    total: files.length,
    truncated: false,
    commitType: null,
    typeReason: null,
    message: null,
    ...overrides,
  };
}
