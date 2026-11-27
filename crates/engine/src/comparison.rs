use std::{collections::BTreeMap, fmt::Write as _};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    Analysis, ContextAddress, Error, Function, ImageIdentity, Instruction, Project, Result,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Changed,
    Moved,
    Added,
    Removed,
    Incomplete,
    Unchanged,
}

impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Changed => "Changed",
            Self::Moved => "Moved",
            Self::Added => "Only after",
            Self::Removed => "Only before",
            Self::Incomplete => "Incomplete",
            Self::Unchanged => "Unchanged",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchBasis {
    SameRvaAndBytes,
    UniqueName,
    UniqueBytes,
    SameRva,
}

impl MatchBasis {
    pub fn label(self) -> &'static str {
        match self {
            Self::SameRvaAndBytes => "Same RVA and bytes",
            Self::UniqueName => "Unique name",
            Self::UniqueBytes => "Unique decoded bytes",
            Self::SameRva => "Same RVA",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FunctionLocation {
    pub address: ContextAddress,
    pub name: String,
    pub instruction_count: usize,
    pub decoded_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct FunctionChange {
    pub kind: ChangeKind,
    pub basis: Option<MatchBasis>,
    pub before: Option<FunctionLocation>,
    pub after: Option<FunctionLocation>,
    pub first_difference: Option<InstructionDifference>,
}

#[derive(Debug, Serialize)]
pub struct InstructionDifference {
    pub before: Option<InstructionLocation>,
    pub after: Option<InstructionLocation>,
}

#[derive(Debug, Serialize)]
pub struct InstructionLocation {
    pub address: ContextAddress,
    pub text: String,
}

impl From<&Instruction> for InstructionLocation {
    fn from(instruction: &Instruction) -> Self {
        Self {
            address: ContextAddress(instruction.address),
            text: instruction.text.clone(),
        }
    }
}

impl InstructionDifference {
    pub fn summary(&self) -> String {
        let display = |location: Option<&InstructionLocation>| {
            location.map_or_else(
                || "—".into(),
                |item| format!("{} {}", item.address, item.text),
            )
        };
        format!(
            "{} → {}",
            display(self.before.as_ref()),
            display(self.after.as_ref())
        )
    }
}

#[derive(Debug, Serialize)]
pub struct BuildComparison {
    pub schema_version: u32,
    pub before: ImageIdentity,
    pub after: ImageIdentity,
    pub notices: Vec<String>,
    pub changes: Vec<FunctionChange>,
}

impl BuildComparison {
    pub fn markdown(&self) -> String {
        let mut output = format!(
            "# Hexaris build comparison\n\nBefore: `{}`\n\nAfter: `{}`\n\n",
            self.before.sha256, self.after.sha256
        );
        for notice in &self.notices {
            let _ = writeln!(output, "- {notice}");
        }
        output.push_str(
            "\n| Result | Before | After | Match evidence | First differing instruction |\n| --- | --- | --- | --- | --- |\n",
        );
        let location = |value: Option<&FunctionLocation>| {
            value.map_or_else(
                || "—".into(),
                |value| {
                    let name = value.name.replace(['\r', '\n', '|', '`', '<', '>'], " ");
                    format!(
                        "`{}` {} ({} instructions)",
                        value.address, name, value.instruction_count
                    )
                },
            )
        };
        for change in self
            .changes
            .iter()
            .filter(|change| change.kind != ChangeKind::Unchanged)
        {
            let _ = writeln!(
                output,
                "| {} | {} | {} | {} | {} |",
                change.kind.label(),
                location(change.before.as_ref()),
                location(change.after.as_ref()),
                change.basis.map_or("—", MatchBasis::label),
                change
                    .first_difference
                    .as_ref()
                    .map_or_else(|| "—".into(), InstructionDifference::summary)
            );
        }
        output
    }
}

struct Candidate<'a> {
    function: &'a Function,
    digest: [u8; 32],
    rva: u64,
}

impl<'a> Candidate<'a> {
    fn new(function: &'a Function, base: u64) -> Self {
        let mut hash = Sha256::new();
        for instruction in &function.instructions {
            hash.update(
                instruction
                    .address
                    .wrapping_sub(function.address)
                    .to_le_bytes(),
            );
            hash.update([instruction.bytes.len() as u8]);
            hash.update(&instruction.bytes);
        }
        Self {
            function,
            digest: hash.finalize().into(),
            rva: function.address.wrapping_sub(base),
        }
    }

    fn complete(&self) -> bool {
        !self.function.instructions.is_empty() && self.function.issues.is_empty()
    }

    fn location(&self) -> FunctionLocation {
        FunctionLocation {
            address: ContextAddress(self.function.address),
            name: self.function.name.clone(),
            instruction_count: self.function.instructions.len(),
            decoded_sha256: self
                .digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        }
    }
}

fn unique_matches<K: Ord>(
    before: &BTreeMap<u64, Candidate<'_>>,
    after: &BTreeMap<u64, Candidate<'_>>,
    key: impl Fn(&Candidate<'_>) -> Option<K>,
) -> Vec<(u64, u64)> {
    let index = |candidates: &BTreeMap<u64, Candidate<'_>>| {
        let mut index = BTreeMap::new();
        for (&address, candidate) in candidates {
            if let Some(key) = key(candidate) {
                index
                    .entry(key)
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(address));
            }
        }
        index
    };
    let before_index = index(before);
    let after_index = index(after);
    before_index
        .into_iter()
        .filter_map(|(key, address)| Some((address?, (*after_index.get(&key)?)?)))
        .collect()
}

fn record_matches(
    before: &mut BTreeMap<u64, Candidate<'_>>,
    after: &mut BTreeMap<u64, Candidate<'_>>,
    pairs: Vec<(u64, u64)>,
    basis: MatchBasis,
    changes: &mut Vec<FunctionChange>,
) {
    for (left, right) in pairs {
        let before = before.remove(&left).expect("unique match");
        let after = after.remove(&right).expect("unique match");
        let kind = if before.digest != after.digest {
            ChangeKind::Changed
        } else if !before.complete() || !after.complete() {
            ChangeKind::Incomplete
        } else if before.rva != after.rva {
            ChangeKind::Moved
        } else {
            ChangeKind::Unchanged
        };
        changes.push(FunctionChange {
            kind,
            basis: Some(basis),
            before: Some(before.location()),
            after: Some(after.location()),
            first_difference: (kind == ChangeKind::Changed)
                .then(|| first_difference(before.function, after.function)),
        });
    }
}

fn first_difference(before: &Function, after: &Function) -> InstructionDifference {
    let index = before
        .instructions
        .iter()
        .zip(&after.instructions)
        .take_while(|(left, right)| {
            left.address.wrapping_sub(before.address) == right.address.wrapping_sub(after.address)
                && left.bytes == right.bytes
        })
        .count();
    InstructionDifference {
        before: before
            .instructions
            .get(index)
            .map(InstructionLocation::from),
        after: after.instructions.get(index).map(InstructionLocation::from),
    }
}

pub fn compare_builds(
    before_project: &Project,
    before_analysis: &Analysis,
    after_project: &Project,
    after_analysis: &Analysis,
) -> Result<BuildComparison> {
    let before = ImageIdentity::from_project(before_project);
    let after = ImageIdentity::from_project(after_project);
    if before.architecture != after.architecture || before.format != after.format {
        return Err(Error::Input(
            "Compare two builds with the same architecture and file format".into(),
        ));
    }
    fn candidates(
        analysis: &Analysis,
        base: Option<ContextAddress>,
    ) -> BTreeMap<u64, Candidate<'_>> {
        analysis
            .functions
            .iter()
            .map(|(&address, function)| {
                (
                    address,
                    Candidate::new(function, base.map_or(0, |base| base.0)),
                )
            })
            .collect()
    }
    let mut left = candidates(before_analysis, before.image_base);
    let mut right = candidates(after_analysis, after.image_base);
    let mut changes = Vec::with_capacity(left.len().max(right.len()));
    let pairs = unique_matches(&left, &right, |candidate| {
        Some((candidate.rva, candidate.digest))
    });
    record_matches(
        &mut left,
        &mut right,
        pairs,
        MatchBasis::SameRvaAndBytes,
        &mut changes,
    );
    let pairs = unique_matches(&left, &right, |candidate| {
        let name = &candidate.function.name;
        (!name.starts_with("sub_") && !name.is_empty()).then(|| name.clone())
    });
    record_matches(
        &mut left,
        &mut right,
        pairs,
        MatchBasis::UniqueName,
        &mut changes,
    );
    let pairs = unique_matches(&left, &right, |candidate| {
        candidate.complete().then_some(candidate.digest)
    });
    record_matches(
        &mut left,
        &mut right,
        pairs,
        MatchBasis::UniqueBytes,
        &mut changes,
    );
    let pairs = unique_matches(&left, &right, |candidate| Some(candidate.rva));
    record_matches(
        &mut left,
        &mut right,
        pairs,
        MatchBasis::SameRva,
        &mut changes,
    );
    changes.extend(left.values().map(|candidate| FunctionChange {
        kind: ChangeKind::Removed,
        basis: None,
        before: Some(candidate.location()),
        after: None,
        first_difference: None,
    }));
    changes.extend(right.values().map(|candidate| FunctionChange {
        kind: ChangeKind::Added,
        basis: None,
        before: None,
        after: Some(candidate.location()),
        first_difference: None,
    }));
    changes.sort_by_key(|change| {
        (
            change.kind as u8,
            change
                .after
                .as_ref()
                .or(change.before.as_ref())
                .map(|entry| entry.address.0),
        )
    });
    let mut notices = vec![
        "Comparison covers discovered code. Function matches are heuristic and byte equality does not establish behavioral equivalence.".into(),
    ];
    notices.extend(
        before_analysis
            .notices
            .iter()
            .map(|notice| format!("Before: {notice}")),
    );
    notices.extend(
        after_analysis
            .notices
            .iter()
            .map(|notice| format!("After: {notice}")),
    );
    Ok(BuildComparison {
        schema_version: 1,
        before,
        after,
        notices,
        changes,
    })
}
