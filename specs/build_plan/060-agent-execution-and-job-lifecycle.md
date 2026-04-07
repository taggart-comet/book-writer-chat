# Build Action 060: Agent Execution And Job Lifecycle

## Goal

Implement prompt construction, Codex CLI execution, job state transitions, revision creation, and result handling for authoring commands.

## Sequencing Note

When an agent implements this action, it should assume all earlier numbered build actions have already been completed and may be relied on as existing project context.

## Scope

This action should implement:

- prompt package construction
- bounded Codex CLI process execution
- job lifecycle persistence and status transitions
- result parsing for success, failure, timeout, and changed files
- revision creation after successful authoring runs
- render refresh after workspace updates
- outbound messenger completion messages with reader links
- the executor interface defined by `codex-cli-execution.md`

## Required Decisions

- define the prompt template and constraint envelope passed to Codex CLI
- choose synchronous versus queued execution for MVP
- define timeout and concurrency policy per conversation
- choose whether the prompt package is passed by stdin or temporary file path within the `codex-cli-execution.md` contract

## MVP Decisions Chosen

- The prompt package is passed to Codex CLI over stdin.
- Authoring execution is synchronous in the request path for MVP.
- Only one authoring job may run at a time per conversation; concurrent jobs for different conversations may proceed independently.
- Per-job timeout is enforced by the backend and the child process is terminated when the timeout is reached.
- A new revision is created only when the authoring run succeeds and render refresh also succeeds.
- Post-run render refresh failure is persisted as `failed`, not `succeeded`, and user-facing messaging should reflect that the workspace changed but the reader refresh did not complete.

## Acceptance Criteria

- A bot-directed authoring command creates a writing job and transitions it through the expected lifecycle.
- Control commands never invoke the agent.
- The prompt package includes user instruction, system constraints, and current book context.
- Successful agent runs create a revision and refresh the latest render state.
- Failed and timed-out runs are persisted distinctly and generate user-facing failure responses.
- One conversation’s workspace cannot be targeted by another conversation’s job.
- Changed files are determined by backend workspace diffing rather than trusting CLI self-reporting.

## Verification

### API Tests

- Add prompt-construction tests to prove required context is included and unrelated data is excluded.
- Add job lifecycle tests for `received`, `accepted`, `running`, `succeeded`, `failed`, and `timed_out`.
- Add result-handling tests proving revisions are created only on successful runs.

### End-To-End API Tests

- Use a controllable fake or test double for Codex CLI to simulate:
  - successful content updates
  - execution failure
  - timeout
- Assert resulting database state, workspace changes, render updates, and outbound messenger messages.

### Frontend Verification

- Use Puppeteer MCP after a successful test job to open the returned reader link and confirm the newly generated content is visible in the UI.
