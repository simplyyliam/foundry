//! The slice dependency graph: layers, dependents, cycles. Pure functions over slices.

use std::collections::HashMap;

use super::Slice;

/// What's derived from the graph, never stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Derived {
    /// Longest path from a root (0 = no dependencies). Slices in one layer can run in
    /// parallel. Slices caught in a cycle (only possible in a hand-edited plan) get 0.
    pub layer: HashMap<String, u32>,
    /// How many slices depend on each slice.
    pub used_by: HashMap<String, u32>,
    pub acyclic: bool,
}

/// Layers by Kahn's algorithm over the known dependencies (unknown ids are ignored).
pub fn derive(slices: &[Slice]) -> Derived {
    let index: HashMap<&str, usize> = slices
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let deps: Vec<Vec<usize>> = slices
        .iter()
        .map(|s| {
            let mut known: Vec<usize> = s
                .depends_on
                .iter()
                .filter_map(|d| index.get(d.as_str()).copied())
                .collect();
            known.sort_unstable();
            known.dedup();
            known
        })
        .collect();
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); slices.len()];
    for (i, ds) in deps.iter().enumerate() {
        for &d in ds {
            dependents[d].push(i);
        }
    }

    let mut waiting: Vec<usize> = deps.iter().map(Vec::len).collect();
    let mut layer = vec![0u32; slices.len()];
    let mut queue: Vec<usize> = (0..slices.len()).filter(|&i| waiting[i] == 0).collect();
    let mut done = 0;
    while let Some(at) = queue.pop() {
        done += 1;
        for &next in &dependents[at] {
            layer[next] = layer[next].max(layer[at] + 1);
            waiting[next] -= 1;
            if waiting[next] == 0 {
                queue.push(next);
            }
        }
    }
    let acyclic = done == slices.len();
    Derived {
        layer: slices
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), if waiting[i] == 0 { layer[i] } else { 0 }))
            .collect(),
        used_by: slices
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.id.clone(),
                    u32::try_from(dependents[i].len()).unwrap_or(u32::MAX),
                )
            })
            .collect(),
        acyclic,
    }
}

/// A cycle through the known dependencies, as slice ids with the first repeated at the end.
pub fn find_cycle(slices: &[Slice]) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Open,
        Done,
    }
    let index: HashMap<&str, usize> = slices
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    fn visit(
        at: usize,
        slices: &[Slice],
        index: &HashMap<&str, usize>,
        marks: &mut [Mark],
        stack: &mut Vec<usize>,
    ) -> Option<Vec<String>> {
        marks[at] = Mark::Open;
        stack.push(at);
        for dep in &slices[at].depends_on {
            let Some(&next) = index.get(dep.as_str()) else {
                continue;
            };
            match marks[next] {
                Mark::Open => {
                    let from = stack.iter().position(|&i| i == next).unwrap_or(0);
                    let mut cycle: Vec<String> = stack[from..]
                        .iter()
                        .map(|&i| slices[i].id.clone())
                        .collect();
                    cycle.push(slices[next].id.clone());
                    return Some(cycle);
                }
                Mark::New => {
                    if let Some(cycle) = visit(next, slices, index, marks, stack) {
                        return Some(cycle);
                    }
                }
                Mark::Done => {}
            }
        }
        stack.pop();
        marks[at] = Mark::Done;
        None
    }
    let mut marks = vec![Mark::New; slices.len()];
    for start in 0..slices.len() {
        if marks[start] == Mark::New {
            if let Some(cycle) = visit(start, slices, &index, &mut marks, &mut Vec::new()) {
                return Some(cycle);
            }
        }
    }
    None
}
