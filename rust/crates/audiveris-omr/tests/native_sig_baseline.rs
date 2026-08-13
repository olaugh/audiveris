//! The port's own SIG, checked against Java's insertion order.
//!
//! A headless STEMS needs the port to answer `edgesOf` itself, which needs it to own a
//! SIG rather than model one as an opaque baseline plus appends. The frozen snapshot
//! `stems-beam-sig-snapshot-chula-system1.txt` is Java's graph in JGraphT insertion
//! order, and that order is stage order -- so the SIG can be grown one stage at a time,
//! each slice checked against its own ordinal range.

use std::path::PathBuf;

use audiveris_image::grid_sig::GridSigNode;
use audiveris_omr::recognize::recognize_grid_lines;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Java's vertex rows, as (ordinal, simple class name, bounds).
fn java_vertices() -> Vec<(usize, String, String)> {
    let text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-sig-snapshot-chula-system1.txt"),
    )
    .expect("frozen SIG snapshot");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("stemsbeamsigsnapshotvertex ") else {
            continue;
        };
        let ordinal: usize = rest
            .split(" ordinal ")
            .nth(1)
            .and_then(|tail| tail.split(' ').next())
            .and_then(|value| value.parse().ok())
            .expect("ordinal");
        let row = rest.split(" row ").nth(1).expect("row");
        let class = row
            .split("org.audiveris.omr.sig.inter.")
            .nth(1)
            .and_then(|tail| tail.split(':').next())
            .expect("class")
            .to_owned();
        let bounds = row
            .split(":bounds=")
            .nth(1)
            .and_then(|tail| tail.split(':').next())
            .unwrap_or("-")
            .to_owned();
        out.push((ordinal, class, bounds));
    }
    out
}

/// Slice 1: GRID's contribution to the SIG, against Java's opening ordinals.
///
/// Java's first 33 vertices are all GRID's -- one brace, 22 barlines, ten connectors --
/// and the port's `GridSig` already holds 32 of them, in the same order, having assigned
/// its own sequential ids. The one it does not hold is the brace at ordinal 0: the port
/// keeps brace inters in a separate store (`brace_sig.rs`), so merging them at the head of
/// the system's vertex list is the first thing a port-owned SIG has to do.
///
/// This pins the agreement that exists and the single gap, so the merge can be written
/// against a failing expectation rather than a guess.
#[test]
fn grid_sig_matches_javas_opening_vertices() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let java = java_vertices();

    let opening: Vec<_> = java
        .iter()
        .take_while(|(_, class, _)| {
            matches!(
                class.as_str(),
                "BraceInter" | "BarlineInter" | "BarConnectorInter"
            )
        })
        .collect();
    assert_eq!(opening.len(), 33, "Java opens with 33 GRID vertices");
    assert_eq!(
        opening[0].1, "BraceInter",
        "the brace is Java's very first vertex"
    );
    let java_barlines = opening
        .iter()
        .filter(|(_, c, _)| c == "BarlineInter")
        .count();
    let java_connectors = opening
        .iter()
        .filter(|(_, c, _)| c == "BarConnectorInter")
        .count();

    let system = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("system 1");
    let nodes: Vec<_> = system.sig.nodes_in_order().collect();
    let barlines = nodes
        .iter()
        .filter(|(_, node)| matches!(node, GridSigNode::Vertical { .. }))
        .count();
    let connectors = nodes
        .iter()
        .filter(|(_, node)| matches!(node, GridSigNode::Connector { .. }))
        .count();

    assert_eq!(
        barlines, java_barlines,
        "the port should hold Java's barline and bracket verticals"
    );
    assert_eq!(connectors, java_connectors, "and Java's connectors");

    // Order, not just census: Java emits every vertical before any connector, and the
    // port's id assignment must agree or the ordinals will not line up once the graph is
    // merged.
    let first_connector = nodes
        .iter()
        .position(|(_, node)| matches!(node, GridSigNode::Connector { .. }))
        .expect("a connector exists");
    assert!(
        nodes[first_connector..]
            .iter()
            .all(|(_, node)| matches!(node, GridSigNode::Connector { .. })),
        "verticals and connectors are interleaved; Java emits all verticals first"
    );
    assert_eq!(
        first_connector, java_barlines,
        "the connector run should start exactly where Java's does"
    );

    // The gap, stated as an assertion so it fails the moment it is closed.
    assert_eq!(
        nodes.len(),
        opening.len() - 1,
        "GridSig should hold every GRID vertex except the brace"
    );
    println!(
        "slice 1: GridSig holds {}/{} of Java's opening vertices in order ({} verticals \
         then {} connectors); the brace at ordinal 0 lives in brace_sig and still needs \
         merging",
        nodes.len(),
        opening.len(),
        barlines,
        connectors
    );
}
