//! The port's own SIG, checked against Java's insertion order.
//!
//! A headless STEMS needs the port to answer `edgesOf` itself, which needs it to own a
//! SIG rather than model one as an opaque baseline plus appends. The frozen snapshot
//! `stems-beam-sig-snapshot-chula-system1.txt` is Java's graph in JGraphT insertion
//! order, and that order is stage order -- so the SIG can be grown one stage at a time,
//! each slice checked against its own ordinal range.

use std::path::PathBuf;

use audiveris_image::grid_sig::GridSigNode;
use audiveris_omr::clef_column::NeutralClefKind;
use audiveris_omr::native_headers::recognize_native_headers;
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

    // The brace, now promoted in production. Java puts it at ordinal 0, before
    // every vertical -- so the merged per-system vertex list is brace first,
    // then GridSig's verticals, then its connectors.
    let promotions: Vec<_> = grid
        .peak_graph
        .brace_promotions
        .iter()
        .filter(|promotion| promotion.system_id == 1)
        .collect();
    assert_eq!(promotions.len(), 1, "chula system 1 has exactly one brace");
    let brace_nodes = grid
        .peak_graph
        .brace_sig
        .system_nodes(1)
        .expect("system 1 brace nodes");
    assert_eq!(brace_nodes.len(), 1, "one BraceInter in the store");

    assert_eq!(
        1 + nodes.len(),
        opening.len(),
        "brace + GridSig should hold every one of Java's opening vertices"
    );
    println!(
        "slice 1: brace + GridSig hold {}/{} of Java's opening vertices in order (brace, \
         {} verticals, {} connectors)",
        1 + nodes.len(),
        opening.len(),
        barlines,
        connectors
    );
}

/// Java's edge rows among a vertex-ordinal range, as (source, target, class).
fn java_edges_within(range: std::ops::RangeInclusive<usize>) -> Vec<(usize, usize, String)> {
    let text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-sig-snapshot-chula-system1.txt"),
    )
    .expect("frozen SIG snapshot");
    let field = |rest: &str, name: &str| -> String {
        rest.split(&format!(" {name} "))
            .nth(1)
            .and_then(|tail| tail.split(' ').next())
            .unwrap_or_else(|| panic!("edge row lacks {name}"))
            .to_owned()
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("stemsbeamsigsnapshotedge ") else {
            continue;
        };
        let source: usize = field(rest, "source").parse().expect("source");
        let target: usize = field(rest, "target").parse().expect("target");
        if range.contains(&source) && range.contains(&target) {
            out.push((source, target, field(rest, "class")));
        }
    }
    out
}

/// Slice 2: HEADERS' contribution, against Java's ordinals 33-42.
///
/// Java inserts header inters in *column* order -- every staff's clef, then per staff the
/// key's alters immediately followed by the key, then every staff's time -- and ties each
/// staff's header together with exactly four relations: `KeyAltersRelation` between
/// neighbouring alters, `Containment` from the key to each alter, and `ClefKeyRelation`
/// from the clef to the key. The times participate in no header-internal relation.
///
/// The port's HEADERS products carry everything that sequence needs: per staff, a selected
/// clef, a selected key whose slices hold the alters in x order, and a selected whole-sign
/// time. This asserts Java's vertex order and edge shape are exactly what those products
/// would derive, which is the rule the merged SIG will use to append its HEADERS slice.
#[test]
fn headers_products_derive_javas_header_ordinals() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");

    // Java's slice, from the frozen snapshot.
    let java = java_vertices();
    let header_range: Vec<_> = java
        .iter()
        .skip_while(|(_, class, _)| {
            matches!(
                class.as_str(),
                "BraceInter" | "BarlineInter" | "BarConnectorInter"
            )
        })
        .take_while(|(_, class, _)| {
            matches!(
                class.as_str(),
                "ClefInter" | "KeyAlterInter" | "KeyInter" | "TimeWholeInter"
            )
        })
        .collect();
    let first_ordinal = header_range.first().expect("headers exist").0;
    let last_ordinal = header_range.last().expect("headers exist").0;
    let java_classes: Vec<&str> = header_range
        .iter()
        .map(|(_, class, _)| class.as_str())
        .collect();

    // The port's system 1 products.
    let system = headers
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("system 1");
    let staffs: Vec<_> = system.staffs.iter().collect();
    assert_eq!(staffs.len(), 2, "chula system 1 is a grand staff");

    // Derive the insertion sequence from the products, in Java's column order.
    let mut derived: Vec<String> = Vec::new();
    for staff in &staffs {
        let clef_id = staff.selected_clef_id.expect("selected clef");
        let clef = staff
            .clef_candidates
            .iter()
            .find(|candidate| candidate.id == clef_id)
            .expect("selected clef exists");
        assert!(
            matches!(clef.kind, NeutralClefKind::Treble),
            "chula's header clefs are G clefs"
        );
        derived.push("ClefInter".to_owned());
    }
    let mut alters_per_staff = Vec::new();
    for staff in &staffs {
        let key_id = staff.selected_key_id.expect("selected key");
        let key = staff
            .key_candidates
            .iter()
            .find(|candidate| candidate.id == key_id)
            .expect("selected key exists");
        let alters = key
            .slices
            .iter()
            .filter(|slice| slice.alter_id.is_some())
            .count();
        alters_per_staff.push(alters);
        for _ in 0..alters {
            derived.push("KeyAlterInter".to_owned());
        }
        derived.push("KeyInter".to_owned());
    }
    for staff in &staffs {
        let time_id = staff.selected_time_id.expect("selected time");
        let time = staff
            .time_candidates
            .iter()
            .find(|candidate| candidate.id == time_id)
            .expect("selected time exists");
        assert!(
            time.member_ids.is_empty(),
            "chula's 2/4 is a whole sign, not a pair"
        );
        derived.push("TimeWholeInter".to_owned());
    }
    assert_eq!(
        derived, java_classes,
        "the products' column-order derivation should reproduce Java's header ordinals"
    );

    // The edge shape, staff by staff. Ordinals within the range follow from the
    // derivation above, so they can be computed rather than searched for.
    let mut expected_edges: Vec<(usize, usize, String)> = Vec::new();
    let mut cursor = first_ordinal + staffs.len(); // first alter, after the clefs
    for (staff_index, alters) in alters_per_staff.iter().enumerate() {
        let clef = first_ordinal + staff_index;
        let first_alter = cursor;
        let key = first_alter + alters;
        for pair in 0..alters.saturating_sub(1) {
            expected_edges.push((
                first_alter + pair,
                first_alter + pair + 1,
                "KeyAltersRelation".to_owned(),
            ));
        }
        for alter in 0..*alters {
            expected_edges.push((key, first_alter + alter, "Containment".to_owned()));
        }
        expected_edges.push((clef, key, "ClefKeyRelation".to_owned()));
        cursor = key + 1;
    }
    let mut java_edges = java_edges_within(first_ordinal..=last_ordinal);
    java_edges.sort();
    expected_edges.sort();
    assert_eq!(
        expected_edges, java_edges,
        "per-staff KeyAlters/Containment/ClefKey should be exactly Java's header edges"
    );
    println!(
        "slice 2: {} header vertices ({java_classes:?}) and {} edges derive exactly",
        derived.len(),
        java_edges.len()
    );
}
