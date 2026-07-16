use indexmap::IndexMap;
use petgraph::graph::NodeIndex;
use std::collections::HashSet;

/// Error type for netlist operations
#[derive(Debug, thiserror::Error)]
pub enum NetlistError {
    #[error("Module not found: {0}")]
    ModuleNotFound(String),
    #[error("Net not found: {0}")]
    NetNotFound(String),
    #[error("Module already exists: {0}")]
    ModuleAlreadyExists(String),
    #[error("Net already exists: {0}")]
    NetAlreadyExists(String),
    #[error("Invalid module name: {0}")]
    InvalidModuleName(String),
    #[error("Invalid net name: {0}")]
    InvalidNetName(String),
}

/// Result type for netlist operations
pub type NetlistResult<T> = Result<T, NetlistError>;

/// A netlist represents a hypergraph used in electronic design automation.
///
/// Modules and nets are identified by integer indices (0..num_modules for modules,
/// 0..num_nets for nets), matching the C++ and Python sibling projects.
/// Names are stored separately for I/O and display purposes.
#[derive(Debug, Clone)]
pub struct Netlist {
    /// Number of I/O pads
    pub num_pads: usize,
    /// Graph representation (nodes are both modules and nets)
    pub gr: petgraph::Graph<(), (), petgraph::Undirected>,
    /// Number of modules
    pub num_modules: usize,
    /// Number of nets
    pub num_nets: usize,
    /// Module names indexed by module index (0..num_modules)
    pub module_names: Vec<String>,
    /// Net names indexed by net index (0..num_nets)
    pub net_names: Vec<String>,
    /// Name → module index lookup
    pub module_map: IndexMap<String, usize>,
    /// Name → net index lookup
    pub net_map: IndexMap<String, usize>,
    /// Petgraph node index for each module (parallel to module_names)
    module_nodes: Vec<NodeIndex>,
    /// Petgraph node index for each net (parallel to net_names)
    net_nodes: Vec<NodeIndex>,
    /// Module weights indexed by module index (default 1)
    pub module_weight: Vec<i32>,
    /// Net weights indexed by net index (default 1)
    pub net_weight: Vec<i32>,
    /// Fixed modules (by module index)
    pub module_fixed: HashSet<usize>,
    /// Whether any modules are fixed
    pub has_fixed_modules: bool,
    /// Cached maximum module degree
    pub max_degree: usize,
    /// Cached maximum net degree
    pub max_net_degree: usize,
}

impl Netlist {
    /// Create a new, empty `Netlist`.
    pub fn new() -> Self {
        Netlist {
            num_pads: 0,
            gr: petgraph::Graph::new_undirected(),
            num_modules: 0,
            num_nets: 0,
            module_names: Vec::new(),
            net_names: Vec::new(),
            module_map: IndexMap::new(),
            net_map: IndexMap::new(),
            module_nodes: Vec::new(),
            net_nodes: Vec::new(),
            module_weight: Vec::new(),
            net_weight: Vec::new(),
            module_fixed: HashSet::new(),
            has_fixed_modules: false,
            max_degree: 0,
            max_net_degree: 0,
        }
    }

    /// Create a new `Netlist` from a graph with given module and net counts.
    ///
    /// Mirrors the C++ constructor: `Netlist(graph_t gr, uint32_t numModules, uint32_t numNets)`.
    /// Modules are assigned indices 0..num_modules, nets num_modules..num_modules+num_nets.
    pub fn from_graph(
        gr: petgraph::Graph<(), (), petgraph::Undirected>,
        num_modules: usize,
        num_nets: usize,
    ) -> Self {
        let total = gr.node_count();
        assert_eq!(
            total,
            num_modules + num_nets,
            "graph node count must equal num_modules + num_nets"
        );

        let module_nodes: Vec<NodeIndex> = (0..num_modules).map(NodeIndex::new).collect();
        let net_nodes: Vec<NodeIndex> = (0..num_nets)
            .map(|i| NodeIndex::new(num_modules + i))
            .collect();

        let module_names: Vec<String> = (0..num_modules).map(|i| format!("m{}", i)).collect();
        let net_names: Vec<String> = (0..num_nets).map(|i| format!("n{}", i)).collect();

        let mut module_map = IndexMap::new();
        for (i, name) in module_names.iter().enumerate() {
            module_map.insert(name.clone(), i);
        }
        let mut net_map = IndexMap::new();
        for (i, name) in net_names.iter().enumerate() {
            net_map.insert(name.clone(), i);
        }

        let mut nl = Netlist {
            num_pads: 0,
            gr,
            num_modules,
            num_nets,
            module_names,
            net_names,
            module_map,
            net_map,
            module_nodes,
            net_nodes,
            module_weight: vec![1; num_modules],
            net_weight: vec![1; num_nets],
            module_fixed: HashSet::new(),
            has_fixed_modules: false,
            max_degree: 0,
            max_net_degree: 0,
        };
        nl.recompute_max_degrees();
        nl
    }

    /// Number of modules.
    pub fn number_of_modules(&self) -> usize {
        self.num_modules
    }

    /// Number of modules (alias).
    pub fn num_modules(&self) -> usize {
        self.num_modules
    }

    /// Number of nets.
    pub fn number_of_nets(&self) -> usize {
        self.num_nets
    }

    /// Number of nets (alias).
    pub fn num_nets(&self) -> usize {
        self.num_nets
    }

    /// Total nodes in the graph (modules + nets).
    pub fn number_of_nodes(&self) -> usize {
        self.gr.node_count()
    }

    /// Add a module with the given name.
    pub fn add_module(&mut self, name: String) -> NetlistResult<usize> {
        if name.is_empty() {
            return Err(NetlistError::InvalidModuleName(name));
        }
        if self.module_map.contains_key(&name) {
            return Err(NetlistError::ModuleAlreadyExists(name));
        }
        let idx = self.num_modules;
        self.module_map.insert(name.clone(), idx);
        self.module_names.push(name);
        let node = self.gr.add_node(());
        self.module_nodes.push(node);
        self.module_weight.push(1);
        self.num_modules += 1;
        Ok(idx)
    }

    /// Add a net with the given name.
    pub fn add_net(&mut self, name: String) -> NetlistResult<usize> {
        if name.is_empty() {
            return Err(NetlistError::InvalidNetName(name));
        }
        if self.net_map.contains_key(&name) {
            return Err(NetlistError::NetAlreadyExists(name));
        }
        let idx = self.num_nets;
        self.net_map.insert(name.clone(), idx);
        self.net_names.push(name);
        let node = self.gr.add_node(());
        self.net_nodes.push(node);
        self.net_weight.push(1);
        self.num_nets += 1;
        Ok(idx)
    }

    /// Add an edge between net `net_idx` and module `module_idx`.
    pub fn add_edge(&mut self, net_idx: usize, module_idx: usize) -> NetlistResult<()> {
        if net_idx >= self.num_nets {
            return Err(NetlistError::NetNotFound(format!("net index {}", net_idx)));
        }
        if module_idx >= self.num_modules {
            return Err(NetlistError::ModuleNotFound(format!(
                "module index {}",
                module_idx
            )));
        }
        let net_node = self.net_nodes[net_idx];
        let mod_node = self.module_nodes[module_idx];
        if self.gr.find_edge(net_node, mod_node).is_none() {
            self.gr.add_edge(net_node, mod_node, ());
            self.update_max_degrees(module_idx, net_idx);
        }
        Ok(())
    }

    /// Get degree of module `module_idx` (number of connected nets).
    pub fn get_module_degree(&self, module_idx: usize) -> usize {
        if module_idx >= self.num_modules {
            return 0;
        }
        self.gr.neighbors(self.module_nodes[module_idx]).count()
    }

    /// Get degree of net `net_idx` (number of connected modules).
    pub fn get_net_degree(&self, net_idx: usize) -> usize {
        if net_idx >= self.num_nets {
            return 0;
        }
        self.gr.neighbors(self.net_nodes[net_idx]).count()
    }

    /// Get module indices connected to net `net_idx`.
    pub fn get_net_modules(&self, net_idx: usize) -> Vec<usize> {
        if net_idx >= self.num_nets {
            return Vec::new();
        }
        let net_node = self.net_nodes[net_idx];
        self.gr
            .neighbors(net_node)
            .filter_map(|n| {
                let idx = n.index();
                if idx < self.num_modules {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get net indices connected to module `module_idx`.
    pub fn get_module_nets(&self, module_idx: usize) -> Vec<usize> {
        if module_idx >= self.num_modules {
            return Vec::new();
        }
        let mod_node = self.module_nodes[module_idx];
        self.gr
            .neighbors(mod_node)
            .filter_map(|n| {
                let idx = n.index() - self.num_modules;
                if idx < self.num_nets {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get module weight (default 1).
    pub fn get_module_weight(&self, module_idx: usize) -> i32 {
        if module_idx < self.module_weight.len() {
            self.module_weight[module_idx]
        } else {
            1
        }
    }

    /// Set module weight.
    pub fn set_module_weight(&mut self, module_idx: usize, weight: i32) {
        if module_idx < self.module_weight.len() {
            self.module_weight[module_idx] = weight;
        }
    }

    /// Get net weight (default 1).
    pub fn get_net_weight(&self, net_idx: usize) -> i32 {
        if net_idx < self.net_weight.len() {
            self.net_weight[net_idx]
        } else {
            1
        }
    }

    /// Set net weight.
    pub fn set_net_weight(&mut self, net_idx: usize, weight: i32) {
        if net_idx < self.net_weight.len() {
            self.net_weight[net_idx] = weight;
        }
    }

    /// Maximum degree among all modules.
    pub fn get_max_degree(&self) -> usize {
        self.max_degree
    }

    /// Maximum degree among all nets.
    pub fn get_max_net_degree(&self) -> usize {
        self.max_net_degree
    }

    /// Look up module index by name.
    pub fn get_module_by_name(&self, name: &str) -> Option<usize> {
        self.module_map.get(name).copied()
    }

    /// Look up net index by name.
    pub fn get_net_by_name(&self, name: &str) -> Option<usize> {
        self.net_map.get(name).copied()
    }

    /// Iterate over all module indices.
    pub fn module_indices(&self) -> impl Iterator<Item = usize> {
        0..self.num_modules
    }

    /// Iterate over all net indices.
    pub fn net_indices(&self) -> impl Iterator<Item = usize> {
        0..self.num_nets
    }

    fn update_max_degrees(&mut self, module_idx: usize, net_idx: usize) {
        let md = self.get_module_degree(module_idx);
        if md > self.max_degree {
            self.max_degree = md;
        }
        let nd = self.get_net_degree(net_idx);
        if nd > self.max_net_degree {
            self.max_net_degree = nd;
        }
    }

    fn recompute_max_degrees(&mut self) {
        self.max_degree = 0;
        self.max_net_degree = 0;
        for m in self.module_indices() {
            let d = self.get_module_degree(m);
            if d > self.max_degree {
                self.max_degree = d;
            }
        }
        for n in self.net_indices() {
            let d = self.get_net_degree(n);
            if d > self.max_net_degree {
                self.max_net_degree = d;
            }
        }
    }
}

impl Default for Netlist {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing `Netlist` instances.
///
/// Accepts string names internally and maps them to integer indices
/// at `build()` time.
pub struct NetlistBuilder {
    netlist: Netlist,
    pending_modules: Vec<String>,
    pending_nets: Vec<String>,
    pending_edges: Vec<(String, String)>,
}

impl NetlistBuilder {
    pub fn new() -> Self {
        Self {
            netlist: Netlist::new(),
            pending_modules: Vec::new(),
            pending_nets: Vec::new(),
            pending_edges: Vec::new(),
        }
    }

    /// Add a module name (will be assigned the next index).
    pub fn add_module(mut self, name: &str) -> Self {
        self.pending_modules.push(name.to_string());
        self
    }

    /// Add a net name (will be assigned the next index).
    pub fn add_net(mut self, name: &str) -> Self {
        self.pending_nets.push(name.to_string());
        self
    }

    /// Add an edge between a net and a module (by name).
    pub fn add_edge(mut self, net: &str, module: &str) -> Self {
        self.pending_edges
            .push((net.to_string(), module.to_string()));
        self
    }

    /// Set the number of pads.
    pub fn with_pads(mut self, num_pads: usize) -> Self {
        self.netlist.num_pads = num_pads;
        self
    }

    /// Build the `Netlist`, resolving names to indices.
    pub fn build(mut self) -> NetlistResult<Netlist> {
        // Add all pending modules
        for name in &self.pending_modules {
            self.netlist.add_module(name.clone())?;
        }
        // Add all pending nets
        for name in &self.pending_nets {
            self.netlist.add_net(name.clone())?;
        }
        // Add all pending edges
        for (net_name, mod_name) in &self.pending_edges {
            let net_idx = self
                .netlist
                .get_net_by_name(net_name)
                .ok_or_else(|| NetlistError::NetNotFound(net_name.clone()))?;
            let mod_idx = self
                .netlist
                .get_module_by_name(mod_name)
                .ok_or_else(|| NetlistError::ModuleNotFound(mod_name.clone()))?;
            self.netlist.add_edge(net_idx, mod_idx)?;
        }
        Ok(self.netlist)
    }
}

impl Default for NetlistBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_netlist() -> Netlist {
        let mut nl = Netlist::new();
        nl.add_module("a0".to_string()).unwrap();
        nl.add_module("a1".to_string()).unwrap();
        nl.add_module("a2".to_string()).unwrap();
        nl.add_net("a3".to_string()).unwrap();
        nl.add_net("a4".to_string()).unwrap();
        nl.add_net("a5".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap(); // a3-a0
        nl.add_edge(0, 1).unwrap(); // a3-a1
        nl.add_edge(2, 0).unwrap(); // a5-a0
        nl.set_module_weight(0, 533);
        nl.set_module_weight(1, 543);
        nl.set_module_weight(2, 532);
        nl
    }

    #[test]
    fn test_create_test_netlist() {
        let nl = create_test_netlist();
        assert_eq!(nl.num_modules(), 3);
        assert_eq!(nl.num_nets(), 3);
        assert_eq!(nl.gr.node_count(), 6);
        assert_eq!(nl.gr.edge_count(), 3);
    }

    #[test]
    fn test_new_netlist() {
        let nl = Netlist::new();
        assert_eq!(nl.num_modules(), 0);
        assert_eq!(nl.num_nets(), 0);
        assert_eq!(nl.gr.node_count(), 0);
        assert_eq!(nl.gr.edge_count(), 0);
    }

    #[test]
    fn test_add_module() {
        let mut nl = Netlist::new();
        let idx = nl.add_module("m1".to_string()).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(nl.num_modules(), 1);
        assert_eq!(nl.module_names[0], "m1");
        assert_eq!(nl.gr.node_count(), 1);
    }

    #[test]
    fn test_add_net() {
        let mut nl = Netlist::new();
        let idx = nl.add_net("n1".to_string()).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(nl.num_nets(), 1);
        assert_eq!(nl.net_names[0], "n1");
        assert_eq!(nl.gr.node_count(), 1);
    }

    #[test]
    fn test_add_edge() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        nl.add_net("n1".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        assert_eq!(nl.gr.edge_count(), 1);
    }

    #[test]
    fn test_get_module_degree() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        nl.add_module("m2".to_string()).unwrap();
        nl.add_net("n1".to_string()).unwrap();
        nl.add_net("n2".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        nl.add_edge(1, 0).unwrap();
        nl.add_edge(0, 1).unwrap();
        assert_eq!(nl.get_module_degree(0), 2);
        assert_eq!(nl.get_module_degree(1), 1);
    }

    #[test]
    fn test_get_net_degree() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        nl.add_module("m2".to_string()).unwrap();
        nl.add_module("m3".to_string()).unwrap();
        nl.add_net("n1".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        nl.add_edge(0, 1).unwrap();
        nl.add_edge(0, 2).unwrap();
        assert_eq!(nl.get_net_degree(0), 3);
    }

    #[test]
    fn test_builder() {
        let nl = NetlistBuilder::new()
            .add_module("m1")
            .add_module("m2")
            .add_net("n1")
            .add_edge("n1", "m1")
            .add_edge("n1", "m2")
            .build()
            .unwrap();
        assert_eq!(nl.num_modules(), 2);
        assert_eq!(nl.num_nets(), 1);
    }

    #[test]
    fn test_builder_with_pads() {
        let nl = NetlistBuilder::new()
            .add_module("m1")
            .with_pads(10)
            .build()
            .unwrap();
        assert_eq!(nl.num_pads, 10);
    }

    #[test]
    fn test_netlist_from_graph() {
        let mut gr = petgraph::Graph::<(), (), petgraph::Undirected>::new_undirected();
        gr.add_node(()); // module 0
        gr.add_node(()); // module 1
        gr.add_node(()); // net 0
        gr.add_edge(NodeIndex::new(2), NodeIndex::new(0), ());
        let nl = Netlist::from_graph(gr, 2, 1);
        assert_eq!(nl.num_modules(), 2);
        assert_eq!(nl.num_nets(), 1);
        assert_eq!(nl.get_module_degree(0), 1);
        assert_eq!(nl.get_module_degree(1), 0);
    }

    #[test]
    fn test_get_net_modules() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        nl.add_module("m2".to_string()).unwrap();
        nl.add_net("n1".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        nl.add_edge(0, 1).unwrap();
        let modules = nl.get_net_modules(0);
        assert_eq!(modules.len(), 2);
        assert!(modules.contains(&0));
        assert!(modules.contains(&1));
    }

    #[test]
    fn test_get_module_nets() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        nl.add_net("n1".to_string()).unwrap();
        nl.add_net("n2".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        nl.add_edge(1, 0).unwrap();
        let nets = nl.get_module_nets(0);
        assert_eq!(nets.len(), 2);
        assert!(nets.contains(&0));
        assert!(nets.contains(&1));
    }

    #[test]
    fn test_get_module_weight() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        assert_eq!(nl.get_module_weight(0), 1);
        nl.set_module_weight(0, 42);
        assert_eq!(nl.get_module_weight(0), 42);
    }

    #[test]
    fn test_get_max_net_degree() {
        let mut nl = Netlist::new();
        nl.add_module("m1".to_string()).unwrap();
        nl.add_module("m2".to_string()).unwrap();
        nl.add_net("n1".to_string()).unwrap();
        nl.add_net("n2".to_string()).unwrap();
        nl.add_edge(0, 0).unwrap();
        nl.add_edge(0, 1).unwrap();
        nl.add_edge(1, 0).unwrap();
        assert_eq!(nl.get_max_net_degree(), 2);
    }

    #[test]
    fn test_get_net_weight() {
        let mut nl = Netlist::new();
        nl.add_net("n1".to_string()).unwrap();
        assert_eq!(nl.get_net_weight(0), 1);
    }

    #[test]
    fn test_default_netlist() {
        let nl: Netlist = Default::default();
        assert_eq!(nl.num_modules(), 0);
        assert_eq!(nl.num_nets(), 0);
    }

    #[test]
    fn test_get_module_degree_nonexistent() {
        let nl = Netlist::new();
        assert_eq!(nl.get_module_degree(0), 0);
    }

    #[test]
    fn test_get_net_degree_nonexistent() {
        let nl = Netlist::new();
        assert_eq!(nl.get_net_degree(0), 0);
    }

    #[test]
    fn test_number_of_nodes() {
        let mut nl = Netlist::new();
        assert_eq!(nl.number_of_nodes(), 0);
        nl.add_module("m1".to_string()).unwrap();
        assert_eq!(nl.number_of_nodes(), 1);
        nl.add_net("n1".to_string()).unwrap();
        assert_eq!(nl.number_of_nodes(), 2);
    }

    #[test]
    fn test_lookup_by_name() {
        let mut nl = Netlist::new();
        nl.add_module("a0".to_string()).unwrap();
        nl.add_module("a1".to_string()).unwrap();
        nl.add_net("n0".to_string()).unwrap();
        assert_eq!(nl.get_module_by_name("a0"), Some(0));
        assert_eq!(nl.get_module_by_name("a1"), Some(1));
        assert_eq!(nl.get_net_by_name("n0"), Some(0));
        assert_eq!(nl.get_module_by_name("nonexistent"), None);
    }
}

#[cfg(test)]
#[cfg(feature = "proptest")]
mod proptest_impls {
    use super::*;
    use proptest::prelude::*;

    fn netlist_strategy() -> impl Strategy<Value = Netlist> {
        (0..20usize, 0..20usize, prop::collection::vec((0..20usize, 0..20usize), 0..50)).prop_map(
            |(num_modules, num_nets, edges)| {
                let mut builder = NetlistBuilder::new();

                for i in 0..num_modules {
                    builder = builder.add_module(&format!("m{}", i));
                }
                for i in 0..num_nets {
                    builder = builder.add_net(&format!("n{}", i));
                }

                for (module_idx, net_idx) in edges {
                    if module_idx < num_modules && net_idx < num_nets {
                        builder = builder
                            .add_edge(&format!("n{}", net_idx), &format!("m{}", module_idx));
                    }
                }

                builder.build().unwrap_or_default()
            },
        )
    }

    proptest! {
        #[test]
        fn qc_netlist_arbitrary_is_valid(netlist in netlist_strategy()) {
            assert!(netlist.number_of_nodes() == netlist.num_modules + netlist.num_nets
                && netlist.module_names.len() == netlist.num_modules
                && netlist.net_names.len() == netlist.num_nets);
        }
    }
}
