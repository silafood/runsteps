/// DAG rendering for `runsteps graph`.
///
/// Builds a petgraph directed graph from config steps + depends_on edges,
/// detects cycles, and renders a simple ASCII representation in topological
/// order.
use anyhow::{anyhow, Result};
use console::style;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::config::{Config, Step};

/// Build a directed graph from the given steps.
///
/// Edges go FROM dependency TO dependent (dep → step), so a topological sort
/// produces dependencies before the steps that need them.
fn build_graph(
    steps: &[Step],
) -> (DiGraph<&str, ()>, Vec<NodeIndex>) {
    let mut g: DiGraph<&str, ()> = DiGraph::new();

    // Insert all nodes first.
    let indices: Vec<NodeIndex> = steps.iter().map(|s| g.add_node(s.name.as_str())).collect();

    // Build a name → NodeIndex lookup.
    let name_to_idx: std::collections::HashMap<&str, NodeIndex> = steps
        .iter()
        .zip(indices.iter())
        .map(|(s, &idx)| (s.name.as_str(), idx))
        .collect();

    // Add edges: dep → step (dependency first).
    for (step, &step_idx) in steps.iter().zip(indices.iter()) {
        for dep in &step.depends_on {
            if let Some(&dep_idx) = name_to_idx.get(dep.as_str()) {
                g.add_edge(dep_idx, step_idx, ());
            }
        }
    }

    (g, indices)
}

/// Reconstruct a cycle path string from the graph and the node where the cycle
/// was detected.
///
/// petgraph's `toposort` returns the node index involved in the cycle but not
/// the full path. We trace the cycle by following outgoing edges from the
/// detected node until we revisit it.
fn format_cycle(g: &DiGraph<&str, ()>, cycle_node: NodeIndex) -> String {
    // DFS to find a cycle path starting and ending at cycle_node.
    let mut path: Vec<&str> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let start = cycle_node;

    fn dfs<'a>(
        g: &'a DiGraph<&'a str, ()>,
        current: NodeIndex,
        start: NodeIndex,
        path: &mut Vec<&'a str>,
        visited: &mut std::collections::HashSet<NodeIndex>,
    ) -> bool {
        path.push(g[current]);
        if current != start || path.len() == 1 {
            visited.insert(current);
        }
        for neighbor in g.neighbors(current) {
            if neighbor == start && path.len() > 1 {
                path.push(g[start]);
                return true;
            }
            if !visited.contains(&neighbor) && dfs(g, neighbor, start, path, visited) {
                return true;
            }
        }
        path.pop();
        false
    }

    if dfs(g, start, start, &mut path, &mut visited) {
        path.join(" → ")
    } else {
        // Fallback: just name the node
        g[cycle_node].to_string()
    }
}

/// Render an ASCII DAG for the given config, optionally filtered by group.
///
/// Returns the rendered string on success, or an error if a cycle is detected.
/// The caller should print the result or print the error message to stderr and
/// exit with code 2.
pub fn render_ascii(config: &Config, filter_group: Option<&str>) -> Result<String> {
    // Optionally restrict to a group + transitive deps.
    let steps: Vec<&Step> = if let Some(group) = filter_group {
        // Collect step names in the group.
        let group_names: std::collections::HashSet<&str> = config
            .steps
            .iter()
            .filter(|s| s.group.as_deref() == Some(group))
            .map(|s| s.name.as_str())
            .collect();

        // Expand with transitive deps.
        let mut included: std::collections::HashSet<&str> = group_names.clone();
        let name_to_step: std::collections::HashMap<&str, &Step> = config
            .steps
            .iter()
            .map(|s| (s.name.as_str(), s))
            .collect();

        let mut worklist: Vec<&str> = included.iter().copied().collect();
        while let Some(name) = worklist.pop() {
            if let Some(step) = name_to_step.get(name) {
                for dep in &step.depends_on {
                    if included.insert(dep.as_str()) {
                        worklist.push(dep.as_str());
                    }
                }
            }
        }

        config
            .steps
            .iter()
            .filter(|s| included.contains(s.name.as_str()))
            .collect()
    } else {
        config.steps.iter().collect()
    };

    if steps.is_empty() {
        return Ok(String::new());
    }

    // Build owned Step slice for graph construction.
    let owned_steps: Vec<Step> = steps.iter().map(|&s| s.clone()).collect();
    let (g, _indices) = build_graph(&owned_steps);

    // Detect cycles.
    if let Err(cycle) = toposort(&g, None) {
        let cycle_str = format_cycle(&g, cycle.node_id());
        return Err(anyhow!("cycle detected: {}", cycle_str));
    }

    // Topological order exists — produce the ASCII output.
    // toposort returns nodes in dependency-first order.
    let topo_nodes = toposort(&g, None).unwrap();

    let mut output = String::new();
    let total = topo_nodes.len();

    for (i, node_idx) in topo_nodes.iter().enumerate() {
        let name = g[*node_idx];
        let step = owned_steps
            .iter()
            .find(|s| s.name == name)
            .expect("step must exist");

        let is_last = i == total - 1;
        let prefix = if is_last { "└─" } else { "├─" };

        // Collect dependency names for display.
        let dep_display = if step.depends_on.is_empty() {
            String::new()
        } else {
            let deps = step
                .depends_on
                .iter()
                .map(|d| d.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(" {}", style(format!("← {}", deps)).dim())
        };

        output.push_str(&format!("{} {}{}\n", prefix, name, dep_display));

        if !is_last {
            output.push_str("│\n");
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Metadata, Step};
    use std::collections::HashMap;

    fn make_meta() -> Metadata {
        Metadata {
            name: "test".to_string(),
            description: None,
            justfile: None,
            working_directory: None,
        }
    }

    fn make_step(name: &str, deps: Vec<&str>) -> Step {
        Step {
            name: name.to_string(),
            description: format!("{} desc", name),
            command: Some("echo".to_string()),
            just_recipe: None,
            just_no_deps: None,
            group: None,
            confirm: false,
            depends_on: deps.into_iter().map(String::from).collect(),
            args: vec![],
            prompts: HashMap::new(),
            raw: false,
            env: HashMap::new(),
            parallel: false,
        }
    }

    #[test]
    fn render_no_deps() {
        let config = Config {
            metadata: make_meta(),
            steps: vec![make_step("setup", vec![]), make_step("deploy", vec![])],
            profiles: HashMap::new(),
        };
        let output = render_ascii(&config, None).unwrap();
        assert!(output.contains("setup"), "setup missing from output");
        assert!(output.contains("deploy"), "deploy missing from output");
    }

    #[test]
    fn render_with_deps() {
        let config = Config {
            metadata: make_meta(),
            steps: vec![
                make_step("build", vec![]),
                make_step("deploy", vec!["build"]),
            ],
            profiles: HashMap::new(),
        };
        let output = render_ascii(&config, None).unwrap();
        assert!(output.contains("build"), "build missing");
        assert!(output.contains("deploy"), "deploy missing");
    }

    #[test]
    fn cycle_detection() {
        let config = Config {
            metadata: make_meta(),
            steps: vec![
                make_step("a", vec!["b"]),
                make_step("b", vec!["a"]),
            ],
            profiles: HashMap::new(),
        };
        let err = render_ascii(&config, None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("cycle detected:"), "expected cycle message, got: {msg}");
    }
}
