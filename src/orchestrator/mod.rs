// Media Generation Orchestrator
// - WorkflowService: interpret → classify → generate → upload → publish → complete
// - StateMachine (MediaGenerationLifecycle — 9 states)
// - tokio::join!(interpret, draft) parallel LLM calls
// - StatusBefore invariant (state tidak boleh mundur)
// - Error recovery: retryable → retry, fatal → FAILED
// - Redis Streams integration (XADD / XREADGROUP / XACK / XCLAIM)

pub mod audit_trail;
pub mod decision;
pub mod lifecycle;
pub mod submission;
pub mod workflow;
