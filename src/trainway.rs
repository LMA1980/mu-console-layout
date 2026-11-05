// SPDX-License-Identifier: LGPL-3.0-only WITH LGPL-3.0-linking-exception
//! Pure deterministic Trainway lane allocator and `YARD +N` overflow pager.
//!
//! Invariant: **existing branch sidings never permute or shift lanes when
//! unrelated branches are created, merged or deleted.** Every surviving
//! branch retains its previous lane index (`1..`), and newly created
//! branches claim the lowest unoccupied positive lane.

use std::collections::{BTreeMap, BTreeSet};

use mu_console_protocol::{Branch, RepoTopology, Train};

/// One allocated siding lane on the Trainway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedLane {
    /// Positive lane index (`1..`; `0` is reserved for the `main` line).
    pub lane: u32,
    pub branch: Branch,
    pub train: Option<Train>,
}

/// Solved Trainway track layout for a given siding capacity and page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainwayLayout {
    /// Persistent `ref_name -> lane` map to pass into the next allocation pass.
    pub lane_map: BTreeMap<String, u32>,
    /// Main-line train, if any.
    pub main_train: Option<Train>,
    /// Sidings visible on the current `yard_page`, ordered by `lane`.
    pub visible_sidings: Vec<AllocatedLane>,
    /// Number of sidings that overflowed the single-page capacity (`YARD +N`).
    pub yard_overflow: u32,
    /// Total number of yard pages (`>= 1`).
    pub total_pages: u32,
    /// Clamped current page (`0..total_pages`).
    pub page: u32,
}

/// Allocate stable siding lanes for `topology` given `previous_lanes`.
///
/// - Surviving branches in `topology.branches` keep their exact lane from
///   `previous_lanes` (if `>= 1`).
/// - New branches (in canonical `ref_name` order) take the lowest free lane
///   in `1..`.
#[must_use]
pub fn allocate_lanes(
    topology: &RepoTopology,
    previous_lanes: &BTreeMap<String, u32>,
    max_visible_sidings: usize,
    yard_page: u32,
) -> TrainwayLayout {
    let mut lane_map = BTreeMap::new();
    let mut used = BTreeSet::new();

    // Pass 1: preserve lanes of surviving branches.
    for b in &topology.branches {
        if let Some(&prev) = previous_lanes.get(&b.ref_name) {
            if prev >= 1 && used.insert(prev) {
                lane_map.insert(b.ref_name.clone(), prev);
            }
        }
    }

    // Pass 2: assign lowest available lane >= 1 to new branches.
    let mut candidate: u32 = 1;
    for b in &topology.branches {
        if lane_map.contains_key(&b.ref_name) {
            continue;
        }
        while used.contains(&candidate) {
            candidate = candidate.saturating_add(1);
        }
        used.insert(candidate);
        lane_map.insert(b.ref_name.clone(), candidate);
        candidate = candidate.saturating_add(1);
    }

    let mut all_sidings: Vec<AllocatedLane> = topology
        .branches
        .iter()
        .map(|b| {
            let lane = lane_map.get(&b.ref_name).copied().unwrap_or(1);
            let train = topology
                .trains
                .iter()
                .filter(|t| t.ref_name == b.ref_name)
                .max_by_key(|t| t.job_id)
                .cloned();
            AllocatedLane {
                lane,
                branch: b.clone(),
                train,
            }
        })
        .collect();
    all_sidings.sort_by_key(|a| a.lane);

    let cap = max_visible_sidings.max(1);
    let total = all_sidings.len();
    let yard_overflow = u32::try_from(total.saturating_sub(cap)).unwrap_or(u32::MAX);
    let total_pages = u32::try_from(total.div_ceil(cap).max(1)).unwrap_or(1);
    let page = yard_page.min(total_pages - 1);
    let start = usize::try_from(page).unwrap_or(0).saturating_mul(cap);
    let visible_sidings = all_sidings.into_iter().skip(start).take(cap).collect();

    let main_train = topology
        .trains
        .iter()
        .filter(|t| t.ref_name == topology.default_branch)
        .max_by_key(|t| t.job_id)
        .cloned();

    TrainwayLayout {
        lane_map,
        main_train,
        visible_sidings,
        yard_overflow,
        total_pages,
        page,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mu_console_protocol::{PrState, RepoId, SidingPhase, StageState, Station};

    fn mk_branch(name: &str) -> Branch {
        Branch {
            ref_name: name.into(),
            base_ref: "main".into(),
            fork_stage: 0,
            pr_state: PrState::Open,
            last_job: None,
            phase: SidingPhase::Active,
        }
    }

    fn mk_topo(branches: Vec<&str>) -> RepoTopology {
        let mut b: Vec<Branch> = branches.into_iter().map(mk_branch).collect();
        b.sort_by(|x, y| x.ref_name.cmp(&y.ref_name));
        RepoTopology {
            repo: RepoId::new("fixture", "superproject"),
            submodule: None,
            default_branch: "main".into(),
            stations: vec![Station {
                id: "fmt".into(),
                label: "FMT".into(),
            }],
            branches: b,
            trains: vec![Train {
                job_id: 1,
                ref_name: "main".into(),
                stage: 0,
                stage_states: vec![StageState::Running],
                node_id: Some("node-a".into()),
                node_lost: false,
            }],
            submodules: Vec::new(),
            fabric_rev: 1,
            submodule_bump_pending: false,
        }
    }

    #[test]
    fn surviving_branches_never_permute_across_additions_and_removals() {
        let t1 = mk_topo(vec!["feat/b", "feat/d", "feat/f"]);
        let l1 = allocate_lanes(&t1, &BTreeMap::new(), 4, 0);
        assert_eq!(l1.lane_map["feat/b"], 1);
        assert_eq!(l1.lane_map["feat/d"], 2);
        assert_eq!(l1.lane_map["feat/f"], 3);

        // Remove feat/d and insert feat/a (alphabetically before feat/b) and feat/z.
        // feat/b MUST stay on lane 1 and feat/f MUST stay on lane 3; feat/a claims hole lane 2!
        let t2 = mk_topo(vec!["feat/a", "feat/b", "feat/f", "feat/z"]);
        let l2 = allocate_lanes(&t2, &l1.lane_map, 4, 0);
        assert_eq!(l2.lane_map["feat/b"], 1, "feat/b must not shift");
        assert_eq!(l2.lane_map["feat/f"], 3, "feat/f must not shift");
        assert_eq!(l2.lane_map["feat/a"], 2, "feat/a fills freed lane 2");
        assert_eq!(l2.lane_map["feat/z"], 4, "feat/z takes next lane 4");
    }

    #[test]
    fn yard_overflow_and_paging() {
        let t = mk_topo(vec![
            "feat/1", "feat/2", "feat/3", "feat/4", "feat/5", "feat/6",
        ]);
        let p0 = allocate_lanes(&t, &BTreeMap::new(), 4, 0);
        assert_eq!(p0.visible_sidings.len(), 4);
        assert_eq!(p0.yard_overflow, 2);
        assert_eq!(p0.total_pages, 2);

        let p1 = allocate_lanes(&t, &p0.lane_map, 4, 1);
        assert_eq!(p1.visible_sidings.len(), 2);
        assert_eq!(p1.visible_sidings[0].branch.ref_name, "feat/5");
    }
}
