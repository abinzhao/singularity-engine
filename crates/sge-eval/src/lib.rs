pub mod case;
pub mod grader;
pub mod metrics;
pub mod runner;
pub mod suite;

pub use case::{Case, CaseResult, ExpectedFinding, FindingMatch, Severity};
pub use grader::{ActualFinding, DeterministicGrader, DeterministicGraderLike};
pub use metrics::{ComparisonOutcome, ContractGates, MetricVector, Objective};
pub use runner::{EvaluationReport, NormalizedSnapshot, RunMeta, SuiteRunner};
pub use suite::Suite;

pub use sge_domain as domain;
pub use sge_protocol as protocol;
