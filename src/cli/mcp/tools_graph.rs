use super::{node_kind_label, node_kind_order};
use needle::{
    analysis,
    graph::{self, CodeGraph, EdgeKind, NodeKind},
};
use serde_json::Value;

pub(super) fn find_callers(args: &Value, graph: &CodeGraph) -> Result<String, String> {
    let name = args["name"].as_str().unwrap_or("");
    if name.is_empty() { return Err("name is required".into()); }

    let targets: Vec<u32> = graph
        .nodes.iter()
        .filter(|n| n.name == name || n.name.ends_with(&format!("::{name}")))
        .map(|n| n.id)
        .collect();

    if targets.is_empty() { return Ok(format!("No symbol named '{name}' found in the graph.")); }

    let callers: Vec<_> = graph
        .edges.iter()
        .filter(|e| matches!(e.kind, EdgeKind::Calls) && targets.contains(&e.to))
        .filter_map(|e| graph.nodes.get(e.from as usize))
        .collect();

    if callers.is_empty() { return Ok(format!("'{name}' is not called by any indexed function.")); }

    let mut out = format!("Callers of '{name}' ({} found):\n\n", callers.len());
    for c in &callers {
        out.push_str(&format!("- **{}** ({}) — {}:{}\n",
            c.name, node_kind_label(&c.kind), c.file_path.replace('\\', "/"), c.line_start));
    }
    Ok(out)
}

pub(super) fn find_callees(args: &Value, graph: &CodeGraph) -> Result<String, String> {
    let name = args["name"].as_str().unwrap_or("");
    if name.is_empty() { return Err("name is required".into()); }

    let sources: Vec<u32> = graph
        .nodes.iter()
        .filter(|n| n.name == name || n.name.ends_with(&format!("::{name}")))
        .map(|n| n.id)
        .collect();

    if sources.is_empty() { return Ok(format!("No symbol named '{name}' found in the graph.")); }

    let callees: Vec<_> = graph
        .edges.iter()
        .filter(|e| matches!(e.kind, EdgeKind::Calls) && sources.contains(&e.from))
        .filter_map(|e| graph.nodes.get(e.to as usize))
        .collect();

    if callees.is_empty() { return Ok(format!("'{name}' does not call any other indexed function.")); }

    let mut out = format!("'{name}' calls {} function(s):\n\n", callees.len());
    for c in &callees {
        out.push_str(&format!("- **{}** ({}) — {}:{}\n",
            c.name, node_kind_label(&c.kind), c.file_path.replace('\\', "/"), c.line_start));
    }
    Ok(out)
}

pub(super) fn get_endpoints(graph: &CodeGraph) -> Result<String, String> {
    let endpoints: Vec<_> = graph.nodes.iter().filter(|n| n.kind == NodeKind::Endpoint).collect();
    if endpoints.is_empty() { return Ok("No API endpoints found in the index.".into()); }

    let mut out = format!("{} API endpoint(s):\n\n", endpoints.len());
    for ep in &endpoints {
        let method = ep.detail.as_deref().unwrap_or("?");
        out.push_str(&format!("- **{}** [{}] — {}:{}\n",
            ep.name, method, ep.file_path.replace('\\', "/"), ep.line_start));
    }
    Ok(out)
}

pub(super) fn get_file_structure(args: &Value, graph: &CodeGraph) -> Result<String, String> {
    let file_query = args["file"].as_str().unwrap_or("");
    if file_query.is_empty() { return Err("file is required".into()); }

    let q = file_query.replace('\\', "/").to_lowercase();

    let module = graph.nodes.iter().find(|n| {
        matches!(n.kind, NodeKind::Module) && {
            let fp = n.file_path.replace('\\', "/").to_lowercase();
            fp == q || fp.ends_with(&format!("/{q}")) || fp.contains(&q)
        }
    });

    let Some(module) = module else {
        return Ok(format!("File '{file_query}' not found in the index."));
    };

    let children: Vec<_> = graph
        .edges.iter()
        .filter(|e| e.from == module.id && matches!(e.kind, EdgeKind::Contains))
        .filter_map(|e| graph.nodes.get(e.to as usize))
        .collect();

    let file = module.file_path.replace('\\', "/");
    let mut out = format!("**{}** ({} definition(s)):\n\n", file, children.len());

    let mut sorted = children.clone();
    sorted.sort_by_key(|n| (node_kind_order(&n.kind), n.line_start));

    for n in sorted {
        let kind   = node_kind_label(&n.kind);
        let detail = n.detail.as_deref().map(|d| format!(" [{d}]")).unwrap_or_default();
        out.push_str(&format!("- L{}-{}: **{}** ({kind}{detail})\n", n.line_start, n.line_end, n.name));
    }
    Ok(out)
}

pub(super) fn get_god_nodes(args: &Value, g: &CodeGraph) -> Result<String, String> {
    let limit = args["limit"].as_u64().unwrap_or(10).min(20) as usize;
    let ranked = graph::compute_god_nodes(g, limit);

    if ranked.is_empty() {
        return Ok("No call-graph edges found. Run `sentinel init` on a codebase with functions.".into());
    }

    let mut out = format!("## God Nodes — Top {} by call-graph degree\n\n", ranked.len());
    out.push_str("These are the most connected symbols: highest-degree nodes are architectural load-bearers.\n\n");

    for (rank, (node_id, degree)) in ranked.iter().enumerate() {
        if let Some(node) = g.nodes.get(*node_id as usize) {
            out.push_str(&format!(
                "{}. **{}** ({}) — degree **{}** — {}:{}\n",
                rank + 1,
                node.name,
                node_kind_label(&node.kind),
                degree,
                node.file_path.replace('\\', "/"),
                node.line_start,
            ));
        }
    }
    Ok(out)
}

pub(super) fn get_communities(g: &CodeGraph) -> Result<String, String> {
    let communities = graph::compute_communities(g);

    let mut by_community: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for (&node_id, &community_id) in &communities {
        by_community.entry(community_id).or_default().push(node_id);
    }

    let mut sorted: Vec<(u32, Vec<u32>)> = by_community.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let total = sorted.len();
    let mut out = format!("## Semantic Communities ({total} clusters)\n\n");
    out.push_str("Detected via label propagation on call + import edges.\n\n");

    for (community_id, node_ids) in &sorted {
        let mut members: Vec<&needle::graph::GraphNode> = node_ids.iter()
            .filter_map(|&id| g.nodes.get(id as usize))
            .filter(|n| !matches!(n.kind, NodeKind::Module))
            .collect();
        if members.is_empty() { continue; }
        members.sort_by(|a, b| a.name.cmp(&b.name));

        let mut file_freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for m in &members { *file_freq.entry(m.file_path.as_str()).or_default() += 1; }
        let rep_file = file_freq.iter().max_by_key(|(_, &c)| c)
            .map(|(&f, _)| f.replace('\\', "/").split('/').last().unwrap_or(f).to_string())
            .unwrap_or_default();

        out.push_str(&format!("### Cluster {} — {} symbols · `{}`\n", community_id, members.len(), rep_file));
        for m in members.iter().take(8) {
            out.push_str(&format!("  - **{}** ({})\n", m.name, node_kind_label(&m.kind)));
        }
        if members.len() > 8 {
            out.push_str(&format!("  - … and {} more\n", members.len() - 8));
        }
        out.push('\n');
    }
    Ok(out)
}

pub(super) fn get_surprises(g: &CodeGraph) -> Result<String, String> {
    let communities = graph::compute_communities(g);
    let surprises   = graph::find_surprise_edges(g, &communities);

    if surprises.is_empty() {
        return Ok("No surprise edges found — the codebase is well-clustered with no unexpected cross-module calls.".into());
    }

    let mut out = format!("## Surprise Edges ({} found)\n\n", surprises.len());
    out.push_str("These are calls that bridge otherwise-disconnected clusters — unexpected dependencies worth reviewing.\n\n");

    for (from_id, to_id) in &surprises {
        let from = g.nodes.get(*from_id as usize);
        let to   = g.nodes.get(*to_id as usize);
        if let (Some(f), Some(t)) = (from, to) {
            let from_file = f.file_path.replace('\\', "/").split('/').last().unwrap_or("").to_string();
            let to_file   = t.file_path.replace('\\', "/").split('/').last().unwrap_or("").to_string();
            out.push_str(&format!(
                "- **{}** (`{}`) → **{}** (`{}`)\n",
                f.name, from_file, t.name, to_file
            ));
        }
    }
    Ok(out)
}

pub(super) fn get_health_score(g: &CodeGraph) -> Result<String, String> {
    let report = analysis::health_score(g);
    let mut out = format!(
        "## Codebase Health: {} (Score {}/100)\n\n",
        report.grade, report.score
    );

    out.push_str("### Penalties\n");
    out.push_str(&format!("- Circular dependencies: -{} pts ({} cycles)\n",
        report.details.circular_dep_penalty, report.circular_deps.len()));
    out.push_str(&format!("- God objects: -{} pts ({} found)\n",
        report.details.god_object_penalty, report.god_objects.len()));
    out.push_str(&format!("- Long files: -{} pts ({} files)\n",
        report.details.long_file_penalty, report.long_files.len()));
    out.push_str(&format!("- Orphaned functions: -{} pts\n", report.details.orphan_penalty));
    out.push_str(&format!("- High coupling: -{} pts (avg {:.1} imports/module)\n\n",
        report.details.coupling_penalty, report.avg_coupling));

    if !report.circular_deps.is_empty() {
        out.push_str("### Circular Dependencies\n");
        for cycle in &report.circular_deps {
            out.push_str(&format!("- {}\n", cycle.join(" → ")));
        }
        out.push('\n');
    }

    if !report.god_objects.is_empty() {
        out.push_str("### God Objects (Top 5)\n");
        for g in report.god_objects.iter().take(5) {
            let file = g.file.replace('\\', "/").split('/').last().unwrap_or("").to_string();
            out.push_str(&format!("- `{}` in {} — {} callers, {} callees\n",
                g.name, file, g.caller_count, g.callee_count));
        }
        out.push('\n');
    }

    if !report.long_files.is_empty() {
        out.push_str("### Long Files\n");
        for f in &report.long_files {
            let name = f.path.replace('\\', "/").split('/').last().unwrap_or("").to_string();
            out.push_str(&format!("- {} ({} lines)\n", name, f.lines));
        }
        out.push('\n');
    }

    if !report.orphaned_functions.is_empty() {
        out.push_str(&format!("### Orphaned Functions ({} total, showing first 10)\n",
            report.orphaned_functions.len()));
        for f in report.orphaned_functions.iter().take(10) {
            out.push_str(&format!("- {}\n", f));
        }
    }

    Ok(out)
}

pub(super) fn blast_radius(args: &Value, g: &CodeGraph) -> Result<String, String> {
    let file = args["file"].as_str().unwrap_or("");
    if file.is_empty() { return Err("file is required".into()); }

    let result = analysis::blast_radius(g, file);

    if result.total_files == 0 {
        return Ok(format!("No downstream dependents found for `{}`.\nEither the file has no callers/importers, or it is not in the graph.", file));
    }

    let mut out = format!(
        "## Blast Radius: `{}`\n\n**Risk Score: {}/100  |  {} file(s) affected**\n\n",
        result.source_file.replace('\\', "/").split('/').last().unwrap_or(&result.source_file),
        result.risk_score,
        result.total_files,
    );

    out.push_str("### Affected Files (by dependency depth)\n");
    for af in result.affected.iter().take(20) {
        let name = af.path.replace('\\', "/").split('/').last().unwrap_or("").to_string();
        out.push_str(&format!("- depth {}: `{}`\n", af.depth, name));
    }
    if result.total_files > 20 {
        out.push_str(&format!("\n…and {} more files.\n", result.total_files - 20));
    }

    Ok(out)
}

pub(super) fn analyze_legacy_code(args: &Value, g: &CodeGraph) -> Result<String, String> {
    let file = args["file"].as_str().unwrap_or("");
    if file.is_empty() { return Err("file is required".into()); }

    let result = analysis::blast_radius(g, file);
    
    let mut out = format!("## Legacy Modernization Plan: `{}`\n\n", file);
    out.push_str("### 1. Decoupling Strategy\n");
    if result.total_files > 10 {
        out.push_str("This file is a **God Object** with high coupling. Before refactoring, extract its interfaces into a separate module.\n");
    } else {
        out.push_str("Coupling is manageable. You can begin encapsulating global state and refactoring function signatures safely.\n");
    }
    
    out.push_str("\n### 2. Immediate Risk (Blast Radius)\n");
    out.push_str(&format!("Refactoring this file will affect **{} downstream files** with a risk score of {}.\n", result.total_files, result.risk_score));
    
    out.push_str("\n### 3. Suggested AI Prompt for Refactoring\n");
    out.push_str("```text\nRefactor this code to use dependency injection, replace global variables with explicit state passing, and extract nested database queries into a repository pattern.\n```\n");
    
    Ok(out)
}

pub(super) fn find_dead_code(g: &CodeGraph) -> Result<String, String> {
    let mut in_degrees = std::collections::HashMap::new();
    for e in &g.edges {
        if matches!(e.kind, EdgeKind::Calls | EdgeKind::Reads | EdgeKind::Writes) {
            *in_degrees.entry(e.to).or_insert(0) += 1;
        }
    }
    
    let mut dead = Vec::new();
    for n in &g.nodes {
        if matches!(n.kind, NodeKind::Function | NodeKind::Method | NodeKind::GlobalState | NodeKind::DatabaseTable) {
            let name_lower = n.name.to_lowercase();
            // Ignore common entry points like main, init, tests
            if name_lower == "main" || name_lower.contains("test") || name_lower == "init" {
                continue;
            }
            if *in_degrees.get(&n.id).unwrap_or(&0) == 0 {
                dead.push(n);
            }
        }
    }
    
    if dead.is_empty() {
        return Ok("No dead code found. All functions have at least one caller.".to_string());
    }
    
    let mut out = format!("## Dead Code Candidates ({} found)\n\n", dead.len());
    out.push_str("These symbols have **0 incoming calls/reads** in the indexed call graph. They are safe candidates for pruning.\n\n");
    
    for n in dead.iter().take(20) {
        let label = node_kind_label(&n.kind);
        out.push_str(&format!("- **{}** ({}) — `{}:{}`\n", n.name, label, n.file_path, n.line_start));
    }
    if dead.len() > 20 {
        out.push_str(&format!("\n...and {} more.\n", dead.len() - 20));
    }
    
    Ok(out)
}

pub(super) fn isolate_microservice(args: &Value, g: &CodeGraph) -> Result<String, String> {
    let cluster_id = args["cluster_id"].as_u64().unwrap_or(u64::MAX) as u32;
    if cluster_id == u32::MAX {
        return Err("cluster_id is required".into());
    }
    
    let communities = graph::compute_communities(g);
    
    let mut internal_nodes = std::collections::HashSet::new();
    for (&node_id, &comm_id) in &communities {
        if comm_id == cluster_id {
            internal_nodes.insert(node_id);
        }
    }
    
    if internal_nodes.is_empty() {
        return Ok(format!("Cluster {} not found or is empty.", cluster_id));
    }
    
    let mut incoming_edges = Vec::new();
    let mut outgoing_edges = Vec::new();
    
    for e in &g.edges {
        if !matches!(e.kind, EdgeKind::Calls) { continue; }
        let from_internal = internal_nodes.contains(&e.from);
        let to_internal = internal_nodes.contains(&e.to);
        
        if from_internal && !to_internal {
            outgoing_edges.push(e);
        } else if !from_internal && to_internal {
            incoming_edges.push(e);
        }
    }
    
    let mut out = format!("## Microservice Isolation Plan: Cluster {}\n\n", cluster_id);
    out.push_str(&format!("**Size:** {} internal nodes\n\n", internal_nodes.len()));
    
    out.push_str("### 1. Inbound API Surface (REST/gRPC Interface)\n");
    out.push_str("Other parts of the monolith call these functions. You must expose these as API endpoints:\n");
    if incoming_edges.is_empty() {
        out.push_str("- *None (No incoming calls)*\n");
    } else {
        let mut unique_in = std::collections::HashSet::new();
        for e in incoming_edges {
            if unique_in.insert(e.to) {
                if let Some(n) = g.nodes.get(e.to as usize) {
                    out.push_str(&format!("- `POST /api/{}` (mapped to {})\n", n.name.to_lowercase(), n.name));
                }
            }
        }
    }
    
    out.push_str("\n### 2. Outbound Dependencies (Mock/Proxy)\n");
    out.push_str("This cluster calls these external monolith functions. You must mock these or call them via API:\n");
    if outgoing_edges.is_empty() {
        out.push_str("- *None (Self-contained!)*\n");
    } else {
        let mut unique_out = std::collections::HashSet::new();
        for e in outgoing_edges {
            if unique_out.insert(e.to) {
                if let Some(n) = g.nodes.get(e.to as usize) {
                    out.push_str(&format!("- `{}` (needs RPC client)\n", n.name));
                }
            }
        }
    }
    
    Ok(out)
}
