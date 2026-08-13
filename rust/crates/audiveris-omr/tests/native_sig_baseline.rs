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
use audiveris_omr::native_stem_seeds::recognize_native_stem_seeds;
use audiveris_omr::recognize::{recognize_grid_lines, recognize_native_beams_with_stem_seeds};

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
            .map(|tail| tail.split(':').take(4).collect::<Vec<_>>().join(":"))
            .unwrap_or_else(|| "-".to_owned());
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

/// Slice 3: BEAMS' contribution, against Java's ordinals 43-110.
///
/// Everything derives from products the recognition already surfaces; nothing needed new
/// recording. The vertex order is browse order (`raw_beams`, where one spot can yield a
/// hook and then a beam) followed by the probed-hooks pass (`hooks`), then the groups.
/// The edges follow three rules, each checked here against Java's own graph:
///
///   * `Containment`: each group contains its members -- `group_memberships` verbatim.
///   * `Exclusion`: a hook and a beam graded from the *same item* are alternative
///     readings, adjacent in browse order (Java inserts OVERLAP exclusions for them).
///   * `BeamBeamRelation`: Java's `BeamGroupInter.addMember` supports the new member
///     against every existing member -- all pairs within a group -- **except** pairs
///     already holding an exclusion. Measured on chula system 1: 54 group pairs, 10 of
///     them excluded, exactly the 44 relations Java's SIG carries.
#[test]
fn beams_products_derive_javas_beam_ordinals() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS recognition");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS recognition");

    // Java's slice.
    let java = java_vertices();
    let first_beam = java
        .iter()
        .position(|(_, class, _)| {
            matches!(
                class.as_str(),
                "BeamInter" | "BeamHookInter" | "SmallBeamInter"
            )
        })
        .expect("beams exist");
    let beam_range: Vec<_> = java[first_beam..]
        .iter()
        .take_while(|(_, class, _)| {
            matches!(
                class.as_str(),
                "BeamInter" | "BeamHookInter" | "SmallBeamInter" | "BeamGroupInter"
            )
        })
        .collect();
    let member_classes: Vec<&str> = beam_range
        .iter()
        .map(|(_, class, _)| class.as_str())
        .take_while(|class| *class != "BeamGroupInter")
        .collect();
    let group_count = beam_range.len() - member_classes.len();
    let first_ordinal = beam_range.first().expect("beams").0;

    // The port's system-1 creation order: browse order, then the probed hooks.
    let system_id = 1;
    let created: Vec<&audiveris_omr::beam_inters::RawBeam> = beams
        .beams_after_multiple_rests
        .iter()
        .filter(|(id, _)| *id == system_id)
        .map(|(_, beam)| beam)
        .chain(
            beams
                .hooks
                .iter()
                .filter(|(id, _)| *id == system_id)
                .map(|(_, beam)| beam),
        )
        .collect();
    let derived_classes: Vec<&str> = created
        .iter()
        .map(|beam| match beam.kind {
            audiveris_omr::beam_inters::BeamKind::Beam => "BeamInter",
            audiveris_omr::beam_inters::BeamKind::Hook => "BeamHookInter",
            audiveris_omr::beam_inters::BeamKind::SmallBeam => "SmallBeamInter",
        })
        .collect();
    if derived_classes != member_classes {
        println!(
            "derived ({}): {}",
            derived_classes.len(),
            derived_classes
                .iter()
                .map(|c| match *c {
                    "BeamHookInter" => 'H',
                    "SmallBeamInter" => 'S',
                    _ => 'B',
                })
                .collect::<String>()
        );
        println!(
            "java    ({}): {}",
            member_classes.len(),
            member_classes
                .iter()
                .map(|c| match *c {
                    "BeamHookInter" => 'H',
                    "SmallBeamInter" => 'S',
                    _ => 'B',
                })
                .collect::<String>()
        );
        panic!("browse order then probed hooks should be Java's insertion order");
    }

    // Exclusions: hook and beam graded from the same item, adjacent in browse order.
    let ordinal_of = |index: usize| first_ordinal + index;
    let mut exclusions = std::collections::BTreeSet::new();
    for (index, pair) in created.windows(2).enumerate() {
        let same_item = pair[0].item == pair[1].item;
        let hook_then_beam = pair[0].kind == audiveris_omr::beam_inters::BeamKind::Hook
            && pair[1].kind != audiveris_omr::beam_inters::BeamKind::Hook;
        if same_item && hook_then_beam {
            exclusions.insert((ordinal_of(index), ordinal_of(index + 1)));
        }
    }
    let java_exclusions: std::collections::BTreeSet<(usize, usize)> =
        java_edges_within(first_ordinal..=beam_range.last().expect("beams").0)
            .into_iter()
            .filter(|(_, _, class)| class == "Exclusion")
            .map(|(source, target, _)| (source.min(target), source.max(target)))
            .collect();
    assert_eq!(
        exclusions, java_exclusions,
        "same-item hook/beam adjacency should be exactly Java's OVERLAP exclusions"
    );

    // Groups: memberships verbatim, in group-creation order.
    let membership = beams
        .group_memberships
        .iter()
        .find(|membership| membership.system_id == system_id)
        .expect("system 1 groups");
    assert_eq!(membership.groups.len(), group_count, "group count");
    let first_group_ordinal = first_ordinal + member_classes.len();
    let java_edges = java_edges_within(first_ordinal..=beam_range.last().expect("beams").0);
    let mut java_members: std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>> =
        std::collections::BTreeMap::new();
    for (source, target, class) in &java_edges {
        if class == "Containment" && *source >= first_group_ordinal {
            java_members.entry(*source).or_default().insert(*target);
        }
    }
    let mut beam_beam: std::collections::BTreeSet<(usize, usize)> = Default::default();
    for (group_index, group) in membership.groups.iter().enumerate() {
        let group_ordinal = first_group_ordinal + group_index;
        let members: std::collections::BTreeSet<usize> =
            group.iter().map(|index| ordinal_of(*index)).collect();
        // Singleton groups also exist as SIG vertices with one containment.
        assert_eq!(
            java_members.get(&group_ordinal),
            Some(&members),
            "group {group_index} members should match Java's containments in creation order"
        );
        // BeamBeamRelation: all pairs, minus excluded ones.
        let ordered: Vec<usize> = members.iter().copied().collect();
        for (index, one) in ordered.iter().enumerate() {
            for two in &ordered[index + 1..] {
                if !exclusions.contains(&(*one, *two)) {
                    beam_beam.insert((*one, *two));
                }
            }
        }
    }
    let java_beam_beam: std::collections::BTreeSet<(usize, usize)> = java_edges
        .iter()
        .filter(|(_, _, class)| class == "BeamBeamRelation")
        .map(|(source, target, _)| (*source.min(target), *source.max(target)))
        .collect();
    assert_eq!(
        beam_beam, java_beam_beam,
        "all group pairs minus exclusions should be exactly Java's BeamBeamRelations"
    );
    println!(
        "slice 3: {} members + {} groups, {} exclusions, {} beam-beam relations derive exactly",
        member_classes.len(),
        group_count,
        exclusions.len(),
        beam_beam.len()
    );
}

/// Slice 4: LEDGERS' contribution, against Java's ordinals 111-118.
///
/// Eight vertices, zero edges -- but the bounds are not glyph bounds. `LedgerInter`
/// computes its bounds from `AreaUtil.horizontalParallelogram(median, thickness)`
/// (`computeArea`, LedgerInter.java:235), so the SIG carries parallelogram geometry and
/// only coincides with the glyph where the ink exactly fills it -- 4 of chula's 8 do,
/// which is what exposed the difference. The port's materialized inters carry the same
/// median and thickness, so Java's integer bounds follow from `Rectangle2D.getBounds()`
/// semantics: floor the min corner, ceil the max.
///
/// The order is *not* by abscissa -- Java creates ledgers per staff, per line index --
/// so this asserts the port's list order, not a sorted comparison.
#[test]
fn ledgers_products_derive_javas_ledger_ordinals() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS recognition");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS recognition");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS recognition");

    let java: Vec<String> = java_vertices()
        .into_iter()
        .filter(|(_, class, _)| class == "LedgerInter")
        .map(|(_, _, bounds)| bounds)
        .collect();

    let derived: Vec<String> = ledgers
        .materializer
        .inters()
        .iter()
        .filter(|inter| inter.system_id == 1 && !inter.removed)
        .map(|inter| {
            let ((x1, y1), (x2, y2)) = inter.median;
            let half = inter.thickness / 2.0;
            let min_x = x1.min(x2).floor() as i64;
            let min_y = (y1.min(y2) - half).floor() as i64;
            let max_x = x1.max(x2).ceil() as i64;
            let max_y = (y1.max(y2) + half).ceil() as i64;
            format!("{min_x}:{min_y}:{}:{}", max_x - min_x, max_y - min_y)
        })
        .collect();
    assert_eq!(
        derived, java,
        "parallelogram bounds over the materialized medians should be Java's ledger vertices"
    );
    println!(
        "slice 4: {} ledgers derive exactly, zero edges",
        derived.len()
    );
}

/// Slice 5: HEADS' contribution, against Java's ordinals 119-220.
///
/// The last and largest slice: 102 heads and 58 head-head OVERLAP exclusions, nothing
/// else. The epilog already records everything -- `heads_in_sig_order` is Java's
/// creation/SIG order, `beam_removed_heads` are the vertices the beam purge later takes
/// back out of the SIG, and each staff's `purge.overlap.decisions` are exactly the pairs
/// Java joined with `sig.insertExclusion(purged, kept, OVERLAP)` when `doRemove` was
/// false (NoteHeadsBuilder.java:975).
#[test]
fn heads_products_derive_javas_head_ordinals() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS recognition");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS recognition");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS recognition");
    let heads = audiveris_omr::native_heads::recognize_native_heads(
        &grid,
        &headers,
        &stem_seeds,
        &beams,
        &ledgers,
    )
    .expect("HEADS recognition");

    // Java's slice.
    let java: Vec<(usize, String, String)> = java_vertices()
        .into_iter()
        .filter(|(_, class, _)| class == "HeadInter")
        .map(|(ordinal, _, bounds)| (ordinal, String::new(), bounds))
        .collect();
    let java_shapes: Vec<String> = {
        let text = std::fs::read_to_string(
            repo_root().join("rust/oracle/stems-beam-sig-snapshot-chula-system1.txt"),
        )
        .expect("frozen SIG snapshot");
        text.lines()
            .filter(|line| {
                line.starts_with("stemsbeamsigsnapshotvertex ") && line.contains("HeadInter")
            })
            .map(|line| {
                line.split(":shape=")
                    .nth(1)
                    .and_then(|tail| tail.split(':').next())
                    .expect("shape")
                    .to_owned()
            })
            .collect()
    };
    let first_ordinal = java.first().expect("heads exist").0;

    // The port's SIG order: creation order, minus the heads the beam purge removed.
    let epilog_system = heads
        .epilog
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("system 1 epilog");
    let staff_system = heads
        .epilog
        .staff_epilog
        .systems
        .iter()
        .find(|system| system.system_id == 1)
        .expect("system 1 staff epilog");
    let removed: std::collections::BTreeSet<(usize, usize)> = epilog_system
        .beam_removed_heads
        .iter()
        .map(|reference| (reference.staff_index, reference.head_index))
        .collect();
    let survivors: Vec<(usize, usize)> = epilog_system
        .heads_in_sig_order
        .iter()
        .map(|reference| (reference.staff_index, reference.head_index))
        .filter(|key| !removed.contains(key))
        .collect();

    let resolve = |key: &(usize, usize)| &staff_system.staffs[key.0].heads[key.1];
    let derived: Vec<(String, String)> = survivors
        .iter()
        .map(|key| {
            let head = resolve(key);
            let shape = match head.shape {
                audiveris_omr::head_template::HeadTemplateShape::NoteheadBlack => "NOTEHEAD_BLACK",
                audiveris_omr::head_template::HeadTemplateShape::NoteheadVoid => "NOTEHEAD_VOID",
                audiveris_omr::head_template::HeadTemplateShape::WholeNote => "WHOLE_NOTE",
                audiveris_omr::head_template::HeadTemplateShape::Breve => "BREVE",
            };
            (
                shape.to_owned(),
                format!(
                    "{}:{}:{}:{}",
                    head.bounds.x, head.bounds.y, head.bounds.width, head.bounds.height
                ),
            )
        })
        .collect();
    let expected: Vec<(String, String)> = java_shapes
        .into_iter()
        .zip(java.iter().map(|(_, _, bounds)| bounds.clone()))
        .collect();
    if derived != expected {
        println!("derived {} heads, java {}", derived.len(), expected.len());
        for (index, (have, want)) in derived.iter().zip(&expected).enumerate() {
            if have != want {
                println!("  first divergence at {index}: port {have:?} java {want:?}");
                break;
            }
        }
        panic!("SIG-order survivors should be Java's head vertices");
    }

    // The exclusions: overlap decisions, mapped from input ordinals to SIG ordinals.
    let ordinal_of: std::collections::BTreeMap<(usize, usize), usize> = survivors
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, first_ordinal + index))
        .collect();
    let mut derived_exclusions = std::collections::BTreeSet::new();
    for (staff_index, staff) in staff_system.staffs.iter().enumerate() {
        for decision in &staff.purge.overlap.decisions {
            // Decision indices are creation indices: the purge walks
            // ordered_indices and resolves each position back before recording.
            let purged = (staff_index, decision.purged_index);
            let kept = (staff_index, decision.kept_index);
            let (Some(one), Some(two)) = (ordinal_of.get(&purged), ordinal_of.get(&kept)) else {
                continue; // an excluded head later beam-removed leaves no SIG edge
            };
            derived_exclusions.insert((*one.min(two), *one.max(two)));
        }
    }
    let java_exclusions: std::collections::BTreeSet<(usize, usize)> =
        java_edges_within(first_ordinal..=java.last().expect("heads").0)
            .into_iter()
            .filter(|(_, _, class)| class == "Exclusion")
            .map(|(source, target, _)| (source.min(target), source.max(target)))
            .collect();
    if derived_exclusions != java_exclusions {
        println!(
            "derived {} exclusions, java {}",
            derived_exclusions.len(),
            java_exclusions.len()
        );
        println!(
            "port-only: {:?}",
            derived_exclusions
                .difference(&java_exclusions)
                .collect::<Vec<_>>()
        );
        println!(
            "java-only: {:?}",
            java_exclusions
                .difference(&derived_exclusions)
                .collect::<Vec<_>>()
        );
        panic!("overlap decisions should be exactly Java's head exclusions");
    }
    println!(
        "slice 5: {} heads and {} exclusions derive exactly",
        derived.len(),
        derived_exclusions.len()
    );
}

/// The capstone gate, being converged: rebuild Java's ordered vertex hash.
///
/// The five slices prove class, order, bounds and edge structure. This renders each
/// vertex's full structural token -- `class:shape=..:grade=<javahex>/<bits>:bounds=..:
/// removed=..:abnormal=..:manual=..:implicit=..:profile=..` plus `median`/`height` for
/// beams -- and requires byte-identity with the snapshot rows, which is what makes the
/// assembled SIG a *product* rather than five proofs. Ignored until every field source is
/// confirmed; run with --ignored to see the first divergence.
///
/// Field sources confirmed so far, from the snapshot inventory:
///   * flags: removed/manual/implicit always false at this baseline, profile always 0,
///     abnormal true exactly for beams, hooks and heads (no stem yet);
///   * BeamGroupInter: shape=null, grade exactly 1.0, bounds = union of member bounds;
///   * BraceInter: grade = Grades.intrinsicRatio * 1 = 0.8;
///   * KeyInter: shape=null, bounds = union of its alters' bounds.
#[test]
#[ignore = "token gate under convergence -- grades for GRID/HEADERS classes not yet wired"]
fn assembled_tokens_rebuild_javas_vertex_hash() {
    let text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-sig-snapshot-chula-system1.txt"),
    )
    .expect("frozen SIG snapshot");
    let java_rows: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("stemsbeamsigsnapshotvertex "))
        .map(|line| line.split(" row ").nth(1).expect("row").trim())
        .collect();
    assert_eq!(java_rows.len(), 221);
    // Assembly rendering to be filled in as each class's grade source is confirmed;
    // compare rendered[i] against java_rows[i] and panic with the first field diff.
    // vertexHash = sha256 over "ordinal:token\n" rows, matching GraphOrder.
    println!("gate skeleton: {} Java rows parsed", java_rows.len());
}

/// Exploratory: do the products' grades bit-match Java's tokens? Prints per class.
#[test]
#[ignore = "exploratory print"]
fn grade_sources_bit_match_java() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS recognition");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS recognition");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS recognition");
    let heads = audiveris_omr::native_heads::recognize_native_heads(
        &grid,
        &headers,
        &stem_seeds,
        &beams,
        &ledgers,
    )
    .expect("HEADS recognition");

    let text = std::fs::read_to_string(
        repo_root().join("rust/oracle/stems-beam-sig-snapshot-chula-system1.txt"),
    )
    .expect("frozen SIG snapshot");
    let java_bits: Vec<(String, u64)> = text
        .lines()
        .filter(|line| line.starts_with("stemsbeamsigsnapshotvertex "))
        .map(|line| {
            let row = line.split(" row ").nth(1).expect("row");
            let class = row
                .split("org.audiveris.omr.sig.inter.")
                .nth(1)
                .and_then(|tail| tail.split(':').next())
                .expect("class")
                .to_owned();
            let bits = row
                .split(":grade=")
                .nth(1)
                .and_then(|tail| tail.split(':').next())
                .and_then(|token| token.split('/').nth(1))
                .map(|hex| u64::from_str_radix(hex, 16).expect("bits"))
                .expect("grade bits");
            (class, bits)
        })
        .collect();
    let bits_for = |class: &str| -> Vec<u64> {
        java_bits
            .iter()
            .filter(|(name, _)| name == class)
            .map(|(_, bits)| *bits)
            .collect()
    };

    // Ledgers, in creation order.
    let derived: Vec<u64> = ledgers
        .materializer
        .inters()
        .iter()
        .filter(|inter| inter.system_id == 1 && !inter.removed)
        .map(|inter| inter.grade.to_bits())
        .collect();
    println!(
        "LedgerInter grades bit-match: {}",
        derived == bits_for("LedgerInter")
    );
    for (have, want) in derived.iter().zip(bits_for("LedgerInter")) {
        println!(
            "   ledger port {:.10} java {:.10} ratio {:.10}",
            f64::from_bits(*have),
            f64::from_bits(want),
            f64::from_bits(want) / f64::from_bits(*have)
        );
        println!(
            "     bits port {have:016x} java {want:016x} ulp {}",
            (*have as i64 - want as i64).abs()
        );
    }

    // Heads, SIG-order survivors.
    let epilog_system = heads
        .epilog
        .systems
        .iter()
        .find(|s| s.system_id == 1)
        .unwrap();
    let staff_system = heads
        .epilog
        .staff_epilog
        .systems
        .iter()
        .find(|s| s.system_id == 1)
        .unwrap();
    let removed: std::collections::BTreeSet<(usize, usize)> = epilog_system
        .beam_removed_heads
        .iter()
        .map(|r| (r.staff_index, r.head_index))
        .collect();
    let head_bits: Vec<u64> = epilog_system
        .heads_in_sig_order
        .iter()
        .map(|r| (r.staff_index, r.head_index))
        .filter(|key| !removed.contains(key))
        .map(|(staff, head)| staff_system.staffs[staff].heads[head].grade_bits)
        .collect();
    println!(
        "HeadInter grades bit-match: {}",
        head_bits == bits_for("HeadInter")
    );

    // Beams+hooks, browse order then probe pass.
    let created: Vec<u64> = beams
        .beams_after_multiple_rests
        .iter()
        .filter(|(id, _)| *id == 1)
        .map(|(_, beam)| beam.grade.to_bits())
        .chain(
            beams
                .hooks
                .iter()
                .filter(|(id, _)| *id == 1)
                .map(|(_, beam)| beam.grade.to_bits()),
        )
        .collect();
    let mut java_beams: Vec<u64> = Vec::new();
    for (class, bits) in &java_bits {
        if class == "BeamInter" || class == "BeamHookInter" {
            java_beams.push(*bits);
        }
    }
    println!("Beam/Hook grades bit-match: {}", created == java_beams);

    // GRID verticals+connectors from GridSig intrinsic grades.
    let system = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .find(|s| s.system_id == 1)
        .unwrap();
    let grid_bits: Vec<u64> = system
        .sig
        .nodes_in_order()
        .map(|(_, node)| node.intrinsic_grade().to_bits())
        .collect();
    let mut java_grid: Vec<u64> = Vec::new();
    for (class, bits) in &java_bits {
        if class == "BarlineInter" || class == "BarConnectorInter" {
            java_grid.push(*bits);
        }
    }
    println!(
        "Barline/Connector grades bit-match: {}",
        grid_bits == java_grid
    );

    // Headers: clefs, alters, keys, times from candidates.
    let hs = headers.systems.iter().find(|s| s.system_id == 1).unwrap();
    let mut header_bits = Vec::new();
    for staff in &hs.staffs {
        let clef = staff.selected_clef_id.unwrap();
        header_bits.push(
            staff
                .clef_candidates
                .iter()
                .find(|c| c.id == clef)
                .unwrap()
                .grade
                .to_bits(),
        );
    }
    for staff in &hs.staffs {
        let key = staff.selected_key_id.unwrap();
        let candidate = staff.key_candidates.iter().find(|c| c.id == key).unwrap();
        for slice in &candidate.slices {
            if slice.alter_id.is_some() {
                // Java `KeyBuilder.applyPitchImpact` does
                // `alter.setGrade(pitchedGrades[i])`, so the alter's SIG grade
                // is exactly its pitched grade under the selected clef. The
                // intrinsicRatio is already inside it, applied when the
                // evaluation became an inter grade.
                header_bits.push(slice.alter_grade.unwrap_or(0.0).to_bits());
            }
        }
        // The key's grade is the plain arithmetic mean of its members' grades,
        // summed in slice order -- measured, not assumed: Java's two alters per
        // staff mean to Java's key grade exactly on both staves. Summing the
        // members here rather than scaling the port's precomputed mean is what
        // makes it bit-exact, since the two associate differently.
        let alters = candidate
            .slices
            .iter()
            .filter_map(|slice| slice.alter_grade)
            .collect::<Vec<_>>();
        let key_grade = alters.iter().sum::<f64>() / alters.len() as f64;
        header_bits.push(key_grade.to_bits());
    }
    for staff in &hs.staffs {
        let time = staff.selected_time_id.unwrap();
        header_bits.push(
            staff
                .time_candidates
                .iter()
                .find(|c| c.id == time)
                .unwrap()
                .grade
                .to_bits(),
        );
    }
    let mut java_headers: Vec<u64> = Vec::new();
    for (class, bits) in &java_bits {
        if matches!(
            class.as_str(),
            "ClefInter" | "KeyAlterInter" | "KeyInter" | "TimeWholeInter"
        ) {
            java_headers.push(*bits);
        }
    }
    let matches = header_bits
        .iter()
        .zip(&java_headers)
        .filter(|(a, b)| a == b)
        .count();
    println!(
        "Header grades bit-match: {matches}/{} (alters intentionally unwired)",
        java_headers.len()
    );
    for (index, (have, want)) in header_bits.iter().zip(&java_headers).enumerate() {
        let port = f64::from_bits(*have);
        let java = f64::from_bits(*want);
        // Signed ulp distance, and the exact ratio. A shared ratio across two
        // alters points at their common scale factor; per-glyph ratios point at
        // per-glyph input, i.e. the measured pitch.
        let ulp = (*have as i64) - (*want as i64);
        println!(
            "   header[{index}] port {port:.17} java {java:.17} match {} ulp {ulp:+} \
             ratio {:.17}",
            have == want,
            port / java,
        );
    }
}

/// Java's `LedgerInter` grades, bit for bit.
///
/// `oracle/ledgers-chula.txt` freezes ledger grades at nine decimals, which
/// cannot distinguish a one-ulp `yRef`. That is precisely how a `LineInfo.yAt`
/// divergence survived every earlier ledger gate: `StaffBoundary::y_at_x_ext`
/// evaluated the staff-line spline by true-curve bisection instead of
/// `GeoPath.yAtX`'s convenience parameter, and the near-cancellation in the
/// two `|y - y_target|` dy checks amplified that into 13-21 ulp on three of
/// these eight grades. So this gate compares raw f64 bit patterns and nothing
/// softer.
#[test]
fn ledger_grades_match_java_bit_for_bit() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");
    let stem_seeds = recognize_native_stem_seeds(&grid, &headers).expect("STEM_SEEDS recognition");
    let beams = recognize_native_beams_with_stem_seeds(&grid, headers.beam_erases(), &stem_seeds)
        .expect("BEAMS recognition");
    let ledgers = audiveris_omr::native_ledgers::recognize_native_ledgers(&grid, &beams)
        .expect("LEDGERS recognition");

    let java_bits: Vec<u64> = {
        let text = std::fs::read_to_string(
            repo_root().join("rust/oracle/stems-beam-sig-snapshot-chula-system1.txt"),
        )
        .expect("snapshot");
        text.lines()
            .filter(|line| {
                line.starts_with("stemsbeamsigsnapshotvertex ") && line.contains("LedgerInter")
            })
            .map(|line| {
                let hex = line
                    .split(":grade=")
                    .nth(1)
                    .and_then(|t| t.split(':').next())
                    .and_then(|t| t.split('/').nth(1))
                    .expect("bits");
                u64::from_str_radix(hex, 16).expect("hex")
            })
            .collect()
    };
    assert_eq!(
        java_bits.len(),
        8,
        "chula system 1 LedgerInter count in the Java snapshot"
    );

    let port: Vec<_> = ledgers
        .materializer
        .inters()
        .iter()
        .filter(|inter| inter.system_id == 1 && !inter.removed)
        .collect();
    assert_eq!(
        port.len(),
        java_bits.len(),
        "port ledger count for chula system 1"
    );

    for (inter, want) in port.iter().zip(&java_bits) {
        assert_eq!(
            inter.grade.to_bits(),
            *want,
            "ledger at x={} grade bits: port {:016x} ({:.17}) vs java {:016x} ({:.17})",
            inter.median.0.0,
            inter.grade.to_bits(),
            inter.grade,
            want,
            f64::from_bits(*want),
        );
    }
}

/// Exploratory: are the staff-line ordinates behind the key alters' measured
/// pitch bit-identical to Java's?
///
/// Java rows are frozen in `oracle/key-alter-pitch.txt`, from
/// `oracle/java/KeyAlterPitchBits.java` (`:app:keyAlterPitchProbe`),
/// which prints `staff.getFirstLine().yAt(x)` and `getLastLine().yAt(x)` at each
/// alter's centroid abscissa. Chula system 1 staff 2's alters are already
/// bit-exact and staff 1's are not, so this asks whether the residue is in the
/// ordinate itself or downstream of it.
#[test]
#[ignore = "exploratory print"]
fn key_alter_line_ordinates_against_java() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    // staff, x, java top bits, java bottom bits
    let rows: [(usize, f64, u64, u64); 4] = [
        (1, 284.0, 0x4075_f7da_4e87_1146, 0x407b_4e53_480e_7bd6),
        (1, 310.0, 0x4075_fa67_a80c_907d, 0x407b_50ff_6581_c007),
        (2, 284.0, 0x4082_8a22_ff35_a8b4, 0x4085_348e_726e_4f03),
        (2, 309.0, 0x4082_8b94_f209_4f20, 0x4085_35fa_f630_c7b4),
    ];
    for (staff_id, x, java_top, java_bottom) in rows {
        let staff = grid
            .staff_lines
            .iter()
            .find(|staff| staff.staff_id == staff_id)
            .expect("staff geometry");
        let top = staff.first_line.y_at_x_ext(x);
        let bottom = staff.last_line.y_at_x_ext(x);
        println!(
            "staff {staff_id} x {x}: top {:016x} java {java_top:016x} ulp {:+} | \
             bottom {:016x} java {java_bottom:016x} ulp {:+}",
            top.to_bits(),
            (top.to_bits() as i64) - (java_top as i64),
            bottom.to_bits(),
            (bottom.to_bits() as i64) - (java_bottom as i64),
        );
    }
}

/// Exploratory: walk the key alter's pitch chain against Java, term by term.
///
/// The line ordinates are already bit-identical (see
/// `key_alter_line_ordinates_against_java`), so this asks which of the next
/// terms diverges: the pitch formula, the measured pitch, or the grade.
/// Java rows are frozen in `oracle/key-alter-pitch.txt`, from
/// `:app:keyAlterPitchProbe` on chula. That file also carries each alter's
/// 110-input `MixGlyphDescriptor` feature vector, which is what the next slice
/// compares to decide between the features and the network arithmetic.
#[test]
#[ignore = "exploratory print"]
fn key_alter_pitch_chain_against_java() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    // staff, centroid x/y, center x/y, java massPitch, java geoPitch, java measuredPitch
    /// staff, centroid x/y, area-centre x/y, then Java's massPitch, geoPitch
    /// and measuredPitch as raw bits.
    type PitchRow = (usize, f64, f64, f64, f64, u64, u64, u64);
    let rows: [PitchRow; 4] = [
        (
            1,
            284.0,
            389.0,
            286.0,
            384.5,
            0xbfdf_21e5_d407_bf6b,
            0xbfed_17a7_855f_d990,
            0x3fc3_dcdf_9c19_d523,
        ),
        (
            1,
            310.0,
            358.0,
            310.5,
            353.5,
            0xc00b_3dfb_7f4a_62e2,
            0xc00e_9dc6_731c_4e4e,
            0xc006_1aff_b185_441e,
        ),
        (
            2,
            284.0,
            630.0,
            285.0,
            624.5,
            0xbfe1_c2ea_5cea_d0e2,
            0xbff1_24fb_90f9_a89a,
            0x3fa4_5145_f494_0e08,
        ),
        (
            2,
            309.0,
            600.0,
            310.0,
            594.5,
            0xc00b_1578_215d_563f,
            0xc00f_3745_9789_752a,
            0xc006_537d_94c5_513a,
        ),
    ];
    // Java `Staff.pitchPositionOf` for a point inside the staff.
    let pitch_at = |staff: &audiveris_omr::recognize::StaffLineGeometry, x: f64, y: f64| -> f64 {
        let top = staff.first_line.y_at_x_ext(x);
        let bottom = staff.last_line.y_at_x_ext(x);
        4.0 * ((2.0 * y) - bottom - top) / (bottom - top)
    };
    for (staff_id, cx, cy, gx, gy, java_mass, java_geo, java_measured) in rows {
        let staff = grid
            .staff_lines
            .iter()
            .find(|staff| staff.staff_id == staff_id)
            .expect("staff geometry");
        let mass = pitch_at(staff, cx, cy);
        let geo = pitch_at(staff, gx, gy);
        println!(
            "staff {staff_id} centroid {cx}: mass {:016x} java {java_mass:016x} ulp {:+} | \
             geo {:016x} java {java_geo:016x} ulp {:+} (java measured {java_measured:016x})",
            mass.to_bits(),
            (mass.to_bits() as i64) - (java_mass as i64),
            geo.to_bits(),
            (geo.to_bits() as i64) - (java_geo as i64),
        );
    }
}

/// Exploratory: is the alters' residual drift in the measured pitch or the grade?
#[test]
#[ignore = "exploratory print"]
fn key_alter_measured_pitch_against_java() {
    use audiveris_music_font::{FLAT_MASS_PITCH_OFFSET, MusicFamily, area_pitch_offset};
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let flat_area_offset = area_pitch_offset(MusicFamily::Bravura, "FLAT").unwrap_or(0.0);
    println!(
        "FLAT_MASS_PITCH_OFFSET {:016x} flat_area_offset {:016x} ({flat_area_offset})",
        FLAT_MASS_PITCH_OFFSET.to_bits(),
        flat_area_offset.to_bits(),
    );
    // staff, centroid x/y, center x/y, java measuredPitch
    let rows: [(usize, f64, f64, f64, f64, u64); 4] = [
        (1, 284.0, 389.0, 286.0, 384.5, 0x3fc3_dcdf_9c19_d523),
        (1, 310.0, 358.0, 310.5, 353.5, 0xc006_1aff_b185_441e),
        (2, 284.0, 630.0, 285.0, 624.5, 0x3fa4_5145_f494_0e08),
        (2, 309.0, 600.0, 310.0, 594.5, 0xc006_537d_94c5_513a),
    ];
    let pitch_at = |staff: &audiveris_omr::recognize::StaffLineGeometry, x: f64, y: f64| -> f64 {
        let top = staff.first_line.y_at_x_ext(x);
        let bottom = staff.last_line.y_at_x_ext(x);
        4.0 * ((2.0 * y) - bottom - top) / (bottom - top)
    };
    for (staff_id, cx, cy, gx, gy, java_measured) in rows {
        let staff = grid
            .staff_lines
            .iter()
            .find(|staff| staff.staff_id == staff_id)
            .expect("staff geometry");
        let mass = pitch_at(staff, cx, cy);
        let geo = pitch_at(staff, gx, gy) + flat_area_offset;
        let measured = 0.5 * ((mass + FLAT_MASS_PITCH_OFFSET) + geo);
        println!(
            "staff {staff_id} centroid {cx}: measured {:016x} java {java_measured:016x} ulp {:+}",
            measured.to_bits(),
            (measured.to_bits() as i64) - (java_measured as i64),
        );
    }
}

/// Exploratory: are the key alters' classifier *inputs* bit-identical to Java?
///
/// The pitch chain is exact on all four chula system-1 alters, and staff 2's
/// grades are exact while staff 1's drift 2-3 ulp, so the residue is the
/// classifier. It has exactly two candidate sources: the feature vector and the
/// sigmoid's `exp`. This compares the features against Java's, frozen in
/// `oracle/key-alter-pitch.txt` from `:app:keyAlterPitchProbe`. If they match,
/// the arithmetic is implicated -- and see HANDOFF before assuming fdlibm for
/// `Math.exp`, which HotSpot intrinsifies.
#[test]
#[ignore = "exploratory print"]
fn key_alter_features_against_java() {
    let grid = recognize_grid_lines(repo_root().join("data/examples/chula.png"))
        .expect("GRID recognition");
    let headers = recognize_native_headers(&grid).expect("HEADERS recognition");

    let text = std::fs::read_to_string(repo_root().join("rust/oracle/key-alter-pitch.txt"))
        .expect("key alter oracle");
    let mut java: std::collections::BTreeMap<String, Vec<u64>> = std::collections::BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("keyalterfeatures ") else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let staff = fields[1];
        let bounds = fields[3];
        let start = fields
            .iter()
            .position(|f| *f == "features")
            .expect("features")
            + 1;
        let bits = fields[start..]
            .iter()
            .map(|hex| u64::from_str_radix(hex, 16).expect("hex"))
            .collect();
        java.insert(format!("{staff}:{bounds}"), bits);
    }
    assert_eq!(java.len(), 12, "chula has 12 key alters in the oracle");

    let mut compared = 0;
    for system in &headers.systems {
        for staff in &system.staffs {
            let Some(key) = staff.selected_key_id else {
                continue;
            };
            let Some(candidate) = staff.key_candidates.iter().find(|c| c.id == key) else {
                continue;
            };
            for slice in &candidate.slices {
                let (Some(bounds), Some(raster)) =
                    (slice.alter_bounds, slice.alter_raster.as_ref())
                else {
                    continue;
                };
                let name = format!("{}:{}:{}", staff.staff_id, bounds.x, bounds.y);
                let Some(want) = java.get(&name) else {
                    println!("   no Java row for {name}");
                    continue;
                };
                let features = audiveris_classifier::mix_glyph_features_from_run_table(
                    raster,
                    (bounds.x, bounds.y),
                    staff.specific_interline,
                )
                .expect("features");
                let diffs: Vec<String> = features
                    .iter()
                    .zip(want)
                    .enumerate()
                    .filter(|(_, (have, want))| have.to_bits() != **want)
                    .map(|(index, (have, want))| {
                        format!(
                            "[{index}] {:016x} vs {want:016x} ulp {:+}",
                            have.to_bits(),
                            (have.to_bits() as i64) - (*want as i64)
                        )
                    })
                    .collect();
                compared += 1;
                if diffs.is_empty() {
                    println!("alter {name}: all {} features bit-exact", features.len());
                } else {
                    println!(
                        "alter {name}: {} of {} features differ",
                        diffs.len(),
                        features.len()
                    );
                    let indices: Vec<usize> = features
                        .iter()
                        .zip(want)
                        .enumerate()
                        .filter(|(_, (have, want))| have.to_bits() != **want)
                        .map(|(index, _)| index)
                        .collect();
                    println!("      differing indices: {indices:?}");
                    for diff in diffs.iter().take(3) {
                        println!("      {diff}");
                    }
                }
            }
        }
    }
    println!("compared {compared} alters against Java");
}
