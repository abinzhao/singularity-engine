use sge_domain::TargetRef;
use sge_protocol::ContractDocument;

use crate::proposal::Proposal;

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnosed {
    pub target: TargetRef,
    pub proposals: Vec<Proposal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Approved {
    pub contract: ContractDocument,
}
