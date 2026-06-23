// smithers-source: local
// smithers-display-name: Rust Quality Audit
// smithers-description: Audit Rust crate API/type quality and produce a prioritized plan.
/** @jsxImportSource smithers-orchestrator */
import { createSmithers, Task } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const inputSchema = z.object({
  prompt: z.string().default("Audit this Rust workspace for crate quality."),
});

const auditSchema = z.object({
  auditReport: z.string(),
  prioritizedPlan: z.array(z.object({
    priority: z.enum(["P0", "P1", "P2", "P3"]),
    crateName: z.string(),
    finding: z.string(),
    proposedChange: z.string(),
    affectedFiles: z.array(z.string()).default([]),
    likelyTests: z.array(z.string()).default([]),
    rationale: z.string(),
  })).default([]),
  highestValueSafeChanges: z.array(z.object({
    crateName: z.string(),
    change: z.string(),
    affectedFiles: z.array(z.string()).default([]),
    likelyTests: z.array(z.string()).default([]),
    whySafe: z.string(),
  })).default([]),
});

const { Workflow, smithers, outputs } = createSmithers({
  input: inputSchema,
  audit: auditSchema,
});

export default smithers((ctx) => (
  <Workflow name="rust-quality-audit">
    <Task id="audit" output={outputs.audit} agent={agents.smart} timeoutMs={1_800_000} heartbeatTimeoutMs={600_000}>
      {ctx.input.prompt}
    </Task>
  </Workflow>
));
