use std::collections::BTreeMap;

use serde::Serialize;

use crate::{Analysis, Reference, ReferenceKind};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CallSite {
    pub caller: u64,
    pub site: u64,
    pub target: u64,
}

#[derive(Default)]
pub struct ReferenceIndex {
    incoming: BTreeMap<u64, Vec<Reference>>,
    outgoing: BTreeMap<u64, Vec<Reference>>,
    callers: BTreeMap<u64, Vec<CallSite>>,
    callees: BTreeMap<u64, Vec<CallSite>>,
    owners: BTreeMap<u64, u64>,
}

impl ReferenceIndex {
    pub fn build(analysis: &Analysis) -> Self {
        let mut index = Self::default();
        for function in analysis.functions.values() {
            for instruction in &function.instructions {
                index
                    .owners
                    .entry(instruction.address)
                    .or_insert(function.address);
            }
            for &reference in &function.references {
                index
                    .incoming
                    .entry(reference.target)
                    .or_default()
                    .push(reference);
                index
                    .outgoing
                    .entry(reference.source)
                    .or_default()
                    .push(reference);
                if reference.kind == ReferenceKind::Call {
                    let call = CallSite {
                        caller: function.address,
                        site: reference.source,
                        target: reference.target,
                    };
                    index
                        .callers
                        .entry(reference.target)
                        .or_default()
                        .push(call);
                    index
                        .callees
                        .entry(function.address)
                        .or_default()
                        .push(call);
                }
            }
        }
        for references in index
            .incoming
            .values_mut()
            .chain(index.outgoing.values_mut())
        {
            references.sort_unstable();
            references.dedup();
        }
        for calls in index.callers.values_mut().chain(index.callees.values_mut()) {
            calls.sort_unstable();
            calls.dedup();
        }
        index
    }

    pub fn owner(&self, address: u64) -> Option<u64> {
        self.owners.get(&address).copied()
    }
    pub fn incoming(&self, address: u64) -> &[Reference] {
        self.incoming.get(&address).map_or(&[], Vec::as_slice)
    }
    pub fn outgoing(&self, address: u64) -> &[Reference] {
        self.outgoing.get(&address).map_or(&[], Vec::as_slice)
    }
    pub fn callers(&self, address: u64) -> &[CallSite] {
        self.callers.get(&address).map_or(&[], Vec::as_slice)
    }
    pub fn callees(&self, address: u64) -> &[CallSite] {
        self.callees.get(&address).map_or(&[], Vec::as_slice)
    }
}
