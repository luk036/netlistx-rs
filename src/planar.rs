//! Planarity testing and planar embedding.
//!
//! Implements the Demoucron-Malgrange-Pertuiset (DMP) path-addition algorithm.
//! Starting from a cycle, the algorithm repeatedly embeds a path of a
//! *fragment* (a piece of the graph not yet embedded) into a face whose
//! boundary contains every attachment vertex of that fragment. A fragment with
//! no admissible face certifies non-planarity.
//!
//! Faces are returned as closed boundary walks. For a 2-connected graph every
//! face is a simple cycle; for a graph with cut vertices a face walk may repeat
//! a vertex. The faces of each connected component are concatenated, so a graph
//! with `C` components that contain edges has `F = E - V + 2C` faces (for a
//! connected graph this is the usual `F = E - V + 2`).
//!
//! Reference: J. A. Bondy and U. S. R. Murty, *Graph Theory with
//! Applications*, Algorithm 9.5 (Demoucron, Malgrange, Pertuiset).

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::collections::{HashMap, HashSet, VecDeque};

type NetGraph = Graph<String, f64, Undirected>;

/// True iff `grph` is planar.
pub fn is_planar(grph: &NetGraph) -> bool {
    planar_faces(grph).is_some()
}

/// All faces (inner faces and outer faces) of a planar embedding of `grph`,
/// or `None` if `grph` is not planar.
pub fn planar_faces(grph: &NetGraph) -> Option<Vec<Vec<NodeIndex>>> {
    let adj = adjacency(grph);
    let n = grph.node_count();
    let mut visited = vec![false; n];
    let mut faces = Vec::new();
    for start in grph.node_indices() {
        if visited[start.index()] {
            continue;
        }
        let comp = component(&adj, start, &mut visited);
        faces.extend(component_faces(&adj, &comp)?);
    }
    Some(faces)
}

fn adjacency(grph: &NetGraph) -> Vec<Vec<NodeIndex>> {
    let n = grph.node_count();
    let mut adj = vec![Vec::new(); n];
    for e in grph.edge_references() {
        let a = e.source();
        let b = e.target();
        if a != b {
            adj[a.index()].push(b);
            adj[b.index()].push(a);
        }
    }
    for list in adj.iter_mut() {
        list.sort();
        list.dedup();
    }
    adj
}

fn component(adj: &[Vec<NodeIndex>], start: NodeIndex, visited: &mut [bool]) -> Vec<NodeIndex> {
    let mut comp = Vec::new();
    let mut queue = VecDeque::new();
    visited[start.index()] = true;
    queue.push_back(start);
    while let Some(u) = queue.pop_front() {
        comp.push(u);
        for &v in &adj[u.index()] {
            if !visited[v.index()] {
                visited[v.index()] = true;
                queue.push_back(v);
            }
        }
    }
    comp
}

fn component_faces(adj: &[Vec<NodeIndex>], comp: &[NodeIndex]) -> Option<Vec<Vec<NodeIndex>>> {
    let edge_count: usize = comp
        .iter()
        .map(|&u| adj[u.index()].iter().filter(|&&v| v > u).count())
        .sum();
    if edge_count == 0 {
        return Some(Vec::new());
    }
    let mut in_comp = vec![false; adj.len()];
    for &u in comp {
        in_comp[u.index()] = true;
    }
    match find_cycle(adj, comp) {
        Some(cycle) => dmp(adj, &cycle, &in_comp),
        None => {
            let mut rot = vec![Vec::new(); adj.len()];
            for &u in comp {
                rot[u.index()] = adj[u.index()].clone();
            }
            Some(faces_of_rotation(&rot))
        }
    }
}

fn find_cycle(adj: &[Vec<NodeIndex>], comp: &[NodeIndex]) -> Option<Vec<NodeIndex>> {
    let in_comp: HashSet<usize> = comp.iter().map(|v| v.index()).collect();
    let mut color = vec![0u8; adj.len()];
    let mut parent = vec![usize::MAX; adj.len()];
    let start = comp[0];
    color[start.index()] = 1;
    let mut stack: Vec<(NodeIndex, usize)> = vec![(start, 0)];
    while let Some(&(u, i)) = stack.last() {
        if i >= adj[u.index()].len() {
            color[u.index()] = 2;
            stack.pop();
            continue;
        }
        stack.last_mut().unwrap().1 += 1;
        let v = adj[u.index()][i];
        if !in_comp.contains(&v.index()) {
            continue;
        }
        match color[v.index()] {
            1 => {
                if v.index() == parent[u.index()] {
                    continue;
                }
                let mut cycle = vec![u];
                let mut x = u.index();
                while x != v.index() {
                    x = parent[x];
                    cycle.push(NodeIndex::new(x));
                }
                return Some(cycle);
            }
            0 => {
                color[v.index()] = 1;
                parent[v.index()] = u.index();
                stack.push((v, 0));
            }
            _ => {}
        }
    }
    None
}

struct Fragment {
    vertices: Vec<NodeIndex>,
    attachments: Vec<NodeIndex>,
}

fn dmp(
    adj: &[Vec<NodeIndex>],
    cycle: &[NodeIndex],
    in_comp: &[bool],
) -> Option<Vec<Vec<NodeIndex>>> {
    let n = adj.len();
    let mut rot: Vec<Vec<NodeIndex>> = vec![Vec::new(); n];
    let mut in_h = vec![false; n];
    let mut h_edges: HashSet<(usize, usize)> = HashSet::new();
    let k = cycle.len();
    for i in 0..k {
        let u = cycle[i];
        rot[u.index()] = vec![cycle[(i + k - 1) % k], cycle[(i + 1) % k]];
        in_h[u.index()] = true;
        h_edges.insert(edge_key_idx(u, cycle[(i + 1) % k]));
    }

    loop {
        let faces = faces_of_rotation(&rot);
        let frags = fragments(adj, &in_h, &h_edges, in_comp);
        if frags.is_empty() {
            return Some(faces);
        }

        let mut vertex_faces: HashMap<usize, HashSet<usize>> = HashMap::new();
        for (fi, face) in faces.iter().enumerate() {
            for &v in face {
                vertex_faces.entry(v.index()).or_default().insert(fi);
            }
        }

        let mut choice: Option<(usize, Vec<usize>)> = None;
        for (fi, frag) in frags.iter().enumerate() {
            let adm = admissible_faces(&vertex_faces, &frag.attachments);
            if adm.is_empty() {
                return None;
            }
            if adm.len() == 1 {
                choice = Some((fi, adm));
                break;
            }
            if choice.is_none() {
                choice = Some((fi, adm));
            }
        }
        let (fi, adm) = choice?;
        let path = fragment_path(adj, &frags[fi], &in_h)?;
        if path.len() < 2 {
            return None;
        }
        let embedded_before = h_edges.len();
        embed_path(&mut rot, &mut in_h, &mut h_edges, &faces[adm[0]], &path);
        if h_edges.len() == embedded_before {
            return None;
        }
    }
}

fn fragments(
    adj: &[Vec<NodeIndex>],
    in_h: &[bool],
    h_edges: &HashSet<(usize, usize)>,
    in_comp: &[bool],
) -> Vec<Fragment> {
    let n = adj.len();
    let mut out = Vec::new();

    for u in 0..n {
        if !in_h[u] || !in_comp[u] {
            continue;
        }
        for &v in &adj[u] {
            if v.index() <= u {
                continue;
            }
            if in_h[v.index()] && !h_edges.contains(&(u, v.index())) {
                out.push(Fragment {
                    vertices: Vec::new(),
                    attachments: vec![NodeIndex::new(u), v],
                });
            }
        }
    }

    let mut seen = vec![false; n];
    for s in 0..n {
        if in_h[s] || seen[s] || !in_comp[s] {
            continue;
        }
        let mut verts = Vec::new();
        let mut queue = VecDeque::new();
        seen[s] = true;
        queue.push_back(NodeIndex::new(s));
        while let Some(u) = queue.pop_front() {
            verts.push(u);
            for &v in &adj[u.index()] {
                if !in_h[v.index()] && !seen[v.index()] {
                    seen[v.index()] = true;
                    queue.push_back(v);
                }
            }
        }
        let mut attachments: Vec<NodeIndex> = Vec::new();
        for &u in &verts {
            for &v in &adj[u.index()] {
                if in_h[v.index()] {
                    attachments.push(v);
                }
            }
        }
        attachments.sort();
        attachments.dedup();
        out.push(Fragment {
            vertices: verts,
            attachments,
        });
    }

    out
}

fn admissible_faces(
    vertex_faces: &HashMap<usize, HashSet<usize>>,
    attachments: &[NodeIndex],
) -> Vec<usize> {
    let mut iter = attachments.iter();
    let first = match iter.next() {
        Some(v) => v.index(),
        None => return Vec::new(),
    };
    let mut acc: Vec<usize> = match vertex_faces.get(&first) {
        Some(set) => set.iter().copied().collect(),
        None => return Vec::new(),
    };
    acc.sort_unstable();
    for v in iter {
        match vertex_faces.get(&v.index()) {
            Some(set) => acc.retain(|f| set.contains(f)),
            None => return Vec::new(),
        }
    }
    acc
}

fn fragment_path(adj: &[Vec<NodeIndex>], frag: &Fragment, in_h: &[bool]) -> Option<Vec<NodeIndex>> {
    if frag.vertices.is_empty() {
        if frag.attachments.len() < 2 {
            return None;
        }
        return Some(vec![frag.attachments[0], frag.attachments[1]]);
    }

    let start = *frag.attachments.first()?;
    if frag.attachments.len() == 1 {
        let next = adj[start.index()]
            .iter()
            .copied()
            .find(|v| !in_h[v.index()])?;
        return Some(vec![start, next]);
    }

    let frag_set: HashSet<usize> = frag.vertices.iter().map(|v| v.index()).collect();
    let att_set: HashSet<usize> = frag.attachments.iter().map(|v| v.index()).collect();
    let mut prev: HashMap<usize, usize> = HashMap::new();
    let mut queue = VecDeque::new();
    queue.push_back(start.index());
    let mut goal = None;
    while let Some(u) = queue.pop_front() {
        if u != start.index() && att_set.contains(&u) {
            goal = Some(u);
            break;
        }
        for &v in &adj[u] {
            let vi = v.index();
            if vi == start.index() || prev.contains_key(&vi) {
                continue;
            }
            let v_in_h = att_set.contains(&vi);
            // An edge between two attachment vertices belongs to a separate
            // edge-fragment; this component fragment must be entered first.
            if att_set.contains(&u) && v_in_h {
                continue;
            }
            if !frag_set.contains(&vi) && !v_in_h {
                continue;
            }
            prev.insert(vi, u);
            queue.push_back(vi);
        }
    }
    let goal = goal?;
    let mut path = vec![NodeIndex::new(goal)];
    let mut x = goal;
    while x != start.index() {
        x = *prev.get(&x)?;
        path.push(NodeIndex::new(x));
    }
    path.reverse();
    Some(path)
}

fn embed_path(
    rot: &mut [Vec<NodeIndex>],
    in_h: &mut [bool],
    h_edges: &mut HashSet<(usize, usize)>,
    face: &[NodeIndex],
    path: &[NodeIndex],
) {
    let p0 = path[0];
    let pk = path[path.len() - 1];
    let pk_in_h = in_h[pk.index()];

    let pos0 = face
        .iter()
        .position(|&x| x == p0)
        .expect("attachment on face");
    let after_p0 = face[(pos0 + 1) % face.len()];
    insert_after(&mut rot[p0.index()], after_p0, path[1]);

    if pk_in_h {
        let posk = face
            .iter()
            .position(|&x| x == pk)
            .expect("attachment on face");
        let after_pk = face[(posk + 1) % face.len()];
        insert_after(&mut rot[pk.index()], after_pk, path[path.len() - 2]);
    }

    for i in 1..path.len() {
        in_h[path[i].index()] = true;
    }
    if pk_in_h {
        for i in 1..path.len() - 1 {
            rot[path[i].index()] = vec![path[i - 1], path[i + 1]];
        }
    } else {
        for i in 1..path.len() {
            rot[path[i].index()] = if i + 1 < path.len() {
                vec![path[i - 1], path[i + 1]]
            } else {
                vec![path[i - 1]]
            };
        }
    }
    for i in 0..path.len() - 1 {
        h_edges.insert(edge_key_idx(path[i], path[i + 1]));
    }
}

fn insert_after(list: &mut Vec<NodeIndex>, target: NodeIndex, item: NodeIndex) {
    match list.iter().position(|&x| x == target) {
        Some(pos) => list.insert(pos + 1, item),
        None => list.push(item),
    }
}

fn faces_of_rotation(rot: &[Vec<NodeIndex>]) -> Vec<Vec<NodeIndex>> {
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut faces = Vec::new();
    for u in 0..rot.len() {
        for &v in &rot[u] {
            let start = (u, v.index());
            if visited.contains(&start) {
                continue;
            }
            let mut face = Vec::new();
            let (mut cu, mut cv) = start;
            loop {
                face.push(NodeIndex::new(cu));
                visited.insert((cu, cv));
                let list = &rot[cv];
                let idx = list
                    .iter()
                    .position(|&x| x.index() == cu)
                    .expect("rotation consistency");
                let w = list[(idx + list.len() - 1) % list.len()].index();
                cu = cv;
                cv = w;
                if (cu, cv) == start {
                    break;
                }
            }
            faces.push(face);
        }
    }
    faces
}

fn edge_key_idx(u: NodeIndex, v: NodeIndex) -> (usize, usize) {
    if u.index() <= v.index() {
        (u.index(), v.index())
    } else {
        (v.index(), u.index())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::Graph;

    fn graph(edges: &[(usize, usize)]) -> NetGraph {
        let mut g: NetGraph = Graph::new_undirected();
        let mut nodes: Vec<NodeIndex> = Vec::new();
        for (u, v) in edges {
            while nodes.len() <= (*u).max(*v) {
                nodes.push(g.add_node(format!("n{}", nodes.len())));
            }
            g.add_edge(nodes[*u], nodes[*v], 1.0);
        }
        g
    }

    fn lens(faces: &[Vec<NodeIndex>]) -> Vec<usize> {
        let mut l: Vec<usize> = faces.iter().map(|f| f.len()).collect();
        l.sort_unstable();
        l
    }

    fn assert_closed_walks(g: &NetGraph, faces: &[Vec<NodeIndex>]) {
        for face in faces {
            assert!(face.len() >= 2, "face too short: {:?}", face);
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                assert!(
                    g.find_edge(a, b).is_some(),
                    "face {:?} has non-adjacent step {} -> {}",
                    face,
                    a.index(),
                    b.index()
                );
            }
        }
    }

    #[test]
    fn empty_graph_has_no_faces() {
        let g: NetGraph = Graph::new_undirected();
        assert_eq!(planar_faces(&g), Some(Vec::new()));
    }

    #[test]
    fn single_node_has_no_faces() {
        let mut g: NetGraph = Graph::new_undirected();
        g.add_node("a".to_string());
        assert_eq!(planar_faces(&g), Some(Vec::new()));
    }

    #[test]
    fn single_edge_has_one_face_of_length_two() {
        let g = graph(&[(0, 1)]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(lens(&f), vec![2]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn path_of_three_edges_has_one_face_of_length_six() {
        let g = graph(&[(0, 1), (1, 2), (2, 3)]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(lens(&f), vec![6]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn triangle_has_two_faces() {
        let g = graph(&[(0, 1), (1, 2), (2, 0)]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(lens(&f), vec![3, 3]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn square_has_two_faces() {
        let g = graph(&[(0, 1), (1, 2), (2, 3), (3, 0)]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(lens(&f), vec![4, 4]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn k4_has_four_triangular_faces() {
        let g = graph(&[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(lens(&f), vec![3, 3, 3, 3]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn grid_3x3_has_five_faces() {
        let mut edges = Vec::new();
        for r in 0..3usize {
            for c in 0..3usize {
                let id = r * 3 + c;
                if c < 2 {
                    edges.push((id, id + 1));
                }
                if r < 2 {
                    edges.push((id, id + 3));
                }
            }
        }
        let g = graph(&edges);
        assert_eq!(g.node_count(), 9);
        assert_eq!(g.edge_count(), 12);
        let f = planar_faces(&g).unwrap();
        assert_eq!(f.len(), 5);
        assert_eq!(lens(&f), vec![4, 4, 4, 4, 8]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn wheel_w4_has_five_faces() {
        let g = graph(&[
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 1),
        ]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(lens(&f), vec![3, 3, 3, 3, 4]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn two_disjoint_triangles_have_four_faces() {
        let g = graph(&[(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)]);
        let f = planar_faces(&g).unwrap();
        assert_eq!(f.len(), 4);
        assert_eq!(lens(&f), vec![3, 3, 3, 3]);
        assert_closed_walks(&g, &f);
    }

    #[test]
    fn k5_is_not_planar() {
        let mut g: NetGraph = Graph::new_undirected();
        let nodes: Vec<NodeIndex> = (0..5).map(|i| g.add_node(format!("n{}", i))).collect();
        for i in 0..5 {
            for j in (i + 1)..5 {
                g.add_edge(nodes[i], nodes[j], 1.0);
            }
        }
        assert!(planar_faces(&g).is_none());
        assert!(!is_planar(&g));
    }

    #[test]
    fn k3_3_is_not_planar() {
        let mut g: NetGraph = Graph::new_undirected();
        let nodes: Vec<NodeIndex> = (0..6).map(|i| g.add_node(format!("n{}", i))).collect();
        for i in 0..3 {
            for j in 3..6 {
                g.add_edge(nodes[i], nodes[j], 1.0);
            }
        }
        assert!(planar_faces(&g).is_none());
        assert!(!is_planar(&g));
    }

    #[test]
    fn euler_holds_for_connected_planar_graphs() {
        let cases: Vec<Vec<(usize, usize)>> = vec![
            vec![(0, 1)],
            vec![(0, 1), (1, 2), (2, 0)],
            vec![(0, 1), (1, 2), (2, 3), (3, 0)],
            vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)],
            vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0), (0, 3)],
            vec![(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)],
        ];
        for edges in cases {
            let g = graph(&edges);
            let f = planar_faces(&g).unwrap();
            let v = g.node_count() as isize;
            let e = g.edge_count() as isize;
            assert_eq!(
                f.len() as isize,
                e - v + 2,
                "Euler failed for {:?} (V={}, E={}, F={})",
                edges,
                v,
                e,
                f.len()
            );
        }
    }

    #[test]
    fn non_planar_subdivision_is_rejected() {
        // K5 with one edge subdivided is still non-planar.
        let mut g: NetGraph = Graph::new_undirected();
        let nodes: Vec<NodeIndex> = (0..6).map(|i| g.add_node(format!("n{}", i))).collect();
        for i in 0..5 {
            for j in (i + 1)..5 {
                if i == 0 && j == 1 {
                    continue;
                }
                g.add_edge(nodes[i], nodes[j], 1.0);
            }
        }
        g.add_edge(nodes[0], nodes[5], 1.0);
        g.add_edge(nodes[5], nodes[1], 1.0);
        assert!(planar_faces(&g).is_none());
    }
}
