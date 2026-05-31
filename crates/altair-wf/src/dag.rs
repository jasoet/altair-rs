//! DAG pattern types and validation (cycle detection, missing-dependency
//! detection, duplicate-name detection).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::traits::{TaskInput, TaskOutput};

/// A single node in a DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAGNode<I> {
    /// Unique node name within the DAG.
    pub name: String,
    /// Activity input.
    pub input: I,
    /// Names of nodes this one depends on. Empty for root nodes.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Input to the DAG pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAGInput<I> {
    /// Nodes. Must be non-empty.
    pub nodes: Vec<DAGNode<I>>,
    /// Abort on first failing node (in dependency order).
    #[serde(default)]
    pub fail_fast: bool,
    /// Reserved for a future parallelism cap; currently not enforced —
    /// independent nodes run as concurrently as the worker allows.
    #[serde(default)]
    pub max_parallel: u32,
}

impl<I: TaskInput> DAGInput<I> {
    /// Validate the DAG: non-empty nodes, unique names, no missing
    /// dependencies, no cycles, and each node's `validate()` succeeds.
    pub fn validate(&self) -> Result<()> {
        if self.nodes.is_empty() {
            return Err(Error::InvalidInput(
                "DAG must contain at least one node".into(),
            ));
        }

        // Unique names + name set.
        let mut names: HashSet<&str> = HashSet::new();
        for node in &self.nodes {
            if node.name.is_empty() {
                return Err(Error::InvalidInput("DAG node name is required".into()));
            }
            if !names.insert(node.name.as_str()) {
                return Err(Error::InvalidInput(format!(
                    "duplicate DAG node name: {}",
                    node.name
                )));
            }
        }

        // Dependency references must exist.
        for node in &self.nodes {
            for dep in &node.dependencies {
                if !names.contains(dep.as_str()) {
                    return Err(Error::InvalidInput(format!(
                        "node '{}' depends on missing node '{}'",
                        node.name, dep
                    )));
                }
            }
        }

        // Per-node input validation.
        for node in &self.nodes {
            node.input
                .validate()
                .map_err(|e| Error::InvalidInput(format!("node '{}': {e}", node.name)))?;
        }

        // Cycle detection via DFS.
        detect_cycles(&self.nodes)
    }

    /// Topologically sort the nodes so dependencies precede dependants.
    /// Used by the DAG pattern at runtime to schedule waves of execution.
    ///
    /// Assumes [`Self::validate`] has already succeeded.
    pub(crate) fn topological_layers(&self) -> Vec<Vec<usize>> {
        let deps: Vec<HashSet<String>> = self
            .nodes
            .iter()
            .map(|n| n.dependencies.iter().cloned().collect())
            .collect();

        let mut layers: Vec<Vec<usize>> = Vec::new();
        let mut done: HashSet<String> = HashSet::new();
        let total = self.nodes.len();
        while done.len() < total {
            let mut layer = Vec::new();
            for (idx, node) in self.nodes.iter().enumerate() {
                if done.contains(&node.name) {
                    continue;
                }
                if deps[idx].iter().all(|d| done.contains(d)) {
                    layer.push(idx);
                }
            }
            if layer.is_empty() {
                // Should be unreachable after validate(), but guard
                // against runaway loops.
                break;
            }
            for &idx in &layer {
                done.insert(self.nodes[idx].name.clone());
            }
            layers.push(layer);
        }
        layers
    }
}

fn detect_cycles<I>(nodes: &[DAGNode<I>]) -> Result<()> {
    let adj: HashMap<&str, &[String]> = nodes
        .iter()
        .map(|n| (n.name.as_str(), n.dependencies.as_slice()))
        .collect();
    let mut state: HashMap<&str, u8> = HashMap::new(); // 0 unvisited, 1 visiting, 2 visited

    for node in nodes {
        if state.get(node.name.as_str()).copied().unwrap_or(0) == 0 {
            dfs(node.name.as_str(), &adj, &mut state)?;
        }
    }
    Ok(())
}

fn dfs<'a>(
    name: &'a str,
    adj: &HashMap<&'a str, &'a [String]>,
    state: &mut HashMap<&'a str, u8>,
) -> Result<()> {
    state.insert(name, 1);
    if let Some(deps) = adj.get(name) {
        for dep in *deps {
            match state.get(dep.as_str()).copied().unwrap_or(0) {
                1 => {
                    return Err(Error::InvalidInput(format!(
                        "circular dependency detected involving node '{dep}'"
                    )));
                }
                0 => {
                    // SAFETY-NOTE: dep is borrowed from `adj`'s key set
                    // via the matching node's `dependencies` slice; the
                    // lifetime ties back to the original `nodes` slice.
                    let dep_static: &'a str = adj
                        .keys()
                        .find(|k| **k == dep.as_str())
                        .copied()
                        .ok_or_else(|| {
                            // validate() already checked for missing deps; the
                            // unreachable here is a defensive guard.
                            Error::InvalidInput(format!("node '{dep}' not found"))
                        })?;
                    dfs(dep_static, adj, state)?;
                }
                _ => {}
            }
        }
    }
    state.insert(name, 2);
    Ok(())
}

/// Result of a single DAG node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult<O> {
    /// Node name.
    pub name: String,
    /// Output if the node succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<O>,
    /// Error message if the node failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Per-node duration.
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    /// Whether the node succeeded (mirrors `result.is_some()` when
    /// activity succeeded AND `is_success()` returned true).
    pub success: bool,
}

/// Aggregated DAG result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAGOutput<O> {
    /// Map of node name → output for nodes that succeeded.
    pub results: HashMap<String, O>,
    /// Per-node detailed results, in execution order.
    pub node_results: Vec<NodeResult<O>>,
    /// Successes.
    pub total_success: usize,
    /// Failures.
    pub total_failed: usize,
    /// Wall-clock duration of the full DAG.
    #[serde(with = "duration_serde")]
    pub total_duration: Duration,
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u64(d.as_millis().try_into().unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(de)?;
        Ok(Duration::from_millis(ms))
    }
}

// Manually implement TaskOutput-shape helpers on NodeResult so DAG
// patterns can compose with the trait at higher layers.
impl<O: TaskOutput> NodeResult<O> {
    /// Borrow the wrapped output.
    #[must_use]
    pub fn result(&self) -> Option<&O> {
        self.result.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Task {
        ok: bool,
    }
    impl TaskInput for Task {
        fn validate(&self) -> Result<()> {
            if self.ok {
                Ok(())
            } else {
                Err(Error::InvalidInput("task not ok".into()))
            }
        }
    }

    fn node(name: &str, deps: &[&str]) -> DAGNode<Task> {
        DAGNode {
            name: name.into(),
            input: Task { ok: true },
            dependencies: deps.iter().map(ToString::to_string).collect(),
        }
    }

    #[test]
    fn empty_dag_rejected() {
        let d: DAGInput<Task> = DAGInput {
            nodes: vec![],
            fail_fast: false,
            max_parallel: 0,
        };
        assert!(matches!(d.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn duplicate_node_rejected() {
        let d = DAGInput {
            nodes: vec![node("a", &[]), node("a", &[])],
            fail_fast: false,
            max_parallel: 0,
        };
        match d.validate() {
            Err(Error::InvalidInput(msg)) => assert!(msg.contains("duplicate")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn missing_dependency_rejected() {
        let d = DAGInput {
            nodes: vec![node("a", &["ghost"])],
            fail_fast: false,
            max_parallel: 0,
        };
        match d.validate() {
            Err(Error::InvalidInput(msg)) => assert!(msg.contains("ghost")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn cycle_rejected() {
        let d = DAGInput {
            nodes: vec![node("a", &["b"]), node("b", &["a"])],
            fail_fast: false,
            max_parallel: 0,
        };
        match d.validate() {
            Err(Error::InvalidInput(msg)) => assert!(msg.contains("circular")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn diamond_dag_validates_and_layers() {
        // a → b, a → c, both → d
        let d = DAGInput {
            nodes: vec![
                node("a", &[]),
                node("b", &["a"]),
                node("c", &["a"]),
                node("d", &["b", "c"]),
            ],
            fail_fast: false,
            max_parallel: 0,
        };
        d.validate().unwrap();
        let layers = d.topological_layers();
        assert_eq!(layers.len(), 3, "diamond has 3 layers");
        assert_eq!(layers[0].len(), 1, "first layer is the root");
        assert_eq!(layers[1].len(), 2, "second layer has b + c");
        assert_eq!(layers[2].len(), 1, "third layer is d");
    }

    #[test]
    fn per_node_input_validation_error_surfaces_with_name() {
        let bad = DAGNode {
            name: "broken".into(),
            input: Task { ok: false },
            dependencies: vec![],
        };
        let d = DAGInput {
            nodes: vec![bad],
            fail_fast: false,
            max_parallel: 0,
        };
        match d.validate() {
            Err(Error::InvalidInput(msg)) => assert!(msg.contains("broken")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }
}
