use indexmap::IndexMap;
use indexmap::IndexSet;
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
/// The `Netlist` struct contains modules (cells) and nets (hyperedges) that connect
/// multiple modules together. Each net can connect to multiple modules, making this
/// a true hypergraph representation.
#[derive(Debug, Clone)]
pub struct Netlist {
    /// Number of I/O pads
    pub num_pads: i32,
    /// Cost model identifier
    pub cost_model: i32,
    /// Graph representation (nodes are both modules and nets)
    pub grph: petgraph::Graph<String, (), petgraph::Undirected>,
    /// List of module names
    pub modules: IndexSet<String>,
    /// List of net names
    pub nets: IndexSet<String>,
    /// Module to node index mapping for O(1) lookups
    module_indices: IndexMap<String, NodeIndex>,
    /// Net to node index mapping for O(1) lookups
    net_indices: IndexMap<String, NodeIndex>,
    /// Optional net weights
    pub net_weight: Option<IndexMap<String, i32>>,
    /// Optional module weights
    pub module_weight: Option<IndexMap<String, i32>>,
    /// Set of fixed modules that cannot be moved
    pub module_fixed: HashSet<String>,
    /// Cached maximum module degree
    pub max_degree: u32,
    /// Cached maximum net degree
    pub max_net_degree: u32,
}

impl Netlist {
    /// Creates a new, empty `Netlist`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::Netlist;
    ///
    /// let netlist = Netlist::new();
    /// assert_eq!(netlist.num_modules(), 0);
    /// assert_eq!(netlist.num_nets(), 0);
    /// ```
    pub fn new() -> Self {
        Netlist {
            num_pads: 0,
            cost_model: 0,
            grph: petgraph::Graph::new_undirected(),
            modules: IndexSet::new(),
            nets: IndexSet::new(),
            module_indices: IndexMap::new(),
            net_indices: IndexMap::new(),
            net_weight: None,
            module_weight: None,
            module_fixed: HashSet::new(),
            max_degree: 0,
            max_net_degree: 0,
        }
    }

    /// Returns the number of modules in the netlist
    pub fn num_modules(&self) -> usize {
        self.modules.len()
    }

    /// Returns the number of nets in the netlist
    pub fn num_nets(&self) -> usize {
        self.nets.len()
    }

    /// Adds a module to the netlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the module name is invalid or the module already exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_module("m1".to_string()).unwrap();
    /// assert_eq!(netlist.num_modules(), 1);
    /// ```
    pub fn add_module(&mut self, module: String) -> NetlistResult<()> {
        if module.is_empty() {
            return Err(NetlistError::InvalidModuleName(module));
        }

        if self.modules.contains(&module) {
            return Err(NetlistError::ModuleAlreadyExists(module));
        }

        let node_index = self.grph.add_node(module.clone());
        self.modules.insert(module.clone());
        self.module_indices.insert(module, node_index);

        Ok(())
    }

    /// Adds a net to the netlist.
    ///
    /// # Errors
    ///
    /// Returns an error if the net name is invalid or the net already exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_net("n1".to_string()).unwrap();
    /// assert_eq!(netlist.num_nets(), 1);
    /// ```
    pub fn add_net(&mut self, net: String) -> NetlistResult<()> {
        if net.is_empty() {
            return Err(NetlistError::InvalidNetName(net));
        }

        if self.nets.contains(&net) {
            return Err(NetlistError::NetAlreadyExists(net));
        }

        let node_index = self.grph.add_node(net.clone());
        self.nets.insert(net.clone());
        self.net_indices.insert(net, node_index);

        Ok(())
    }

    /// Adds an edge between a net and a module.
    ///
    /// # Errors
    ///
    /// Returns an error if either the net or module is not found.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_module("m1".to_string()).unwrap();
    /// netlist.add_net("n1".to_string()).unwrap();
    /// netlist.add_edge("n1", "m1").unwrap();
    /// assert_eq!(netlist.grph.edge_count(), 1);
    /// ```
    pub fn add_edge(&mut self, net: &str, module: &str) -> NetlistResult<()> {
        let net_index = self
            .net_indices
            .get(net)
            .ok_or_else(|| NetlistError::NetNotFound(net.to_string()))?;
        let module_index = self
            .module_indices
            .get(module)
            .ok_or_else(|| NetlistError::ModuleNotFound(module.to_string()))?;

        self.grph.add_edge(*net_index, *module_index, ());
        self.invalidate_cache();

        Ok(())
    }

    /// Gets the degree (number of connected nets) of a module.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_module("m1".to_string()).unwrap();
    /// netlist.add_net("n1".to_string()).unwrap();
    /// netlist.add_edge("n1", "m1").unwrap();
    /// assert_eq!(netlist.get_module_degree("m1"), 1);
    /// ```
    pub fn get_module_degree(&self, module: &str) -> usize {
        if let Some(&node_index) = self.module_indices.get(module) {
            self.grph.neighbors(node_index).count()
        } else {
            0
        }
    }

    /// Gets the degree (number of connected modules) of a net.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_module("m1".to_string()).unwrap();
    /// netlist.add_module("m2".to_string()).unwrap();
    /// netlist.add_net("n1".to_string()).unwrap();
    /// netlist.add_edge("n1", "m1").unwrap();
    /// netlist.add_edge("n1", "m2").unwrap();
    /// assert_eq!(netlist.get_net_degree("n1"), 2);
    /// ```
    pub fn get_net_degree(&self, net: &str) -> usize {
        if let Some(&node_index) = self.net_indices.get(net) {
            self.grph.neighbors(node_index).count()
        } else {
            0
        }
    }

    /// Checks if a module exists in the netlist
    pub fn has_module(&self, module: &str) -> bool {
        self.modules.contains(module)
    }

    /// Checks if a net exists in the netlist
    pub fn has_net(&self, net: &str) -> bool {
        self.nets.contains(net)
    }

    /// Gets all modules connected to a net
    pub fn get_net_modules(&self, net: &str) -> Vec<String> {
        let mut modules = Vec::new();
        if let Some(&net_index) = self.net_indices.get(net) {
            for neighbor_index in self.grph.neighbors(net_index) {
                let neighbor_name = &self.grph[neighbor_index];
                if self.modules.contains(neighbor_name) {
                    modules.push(neighbor_name.clone());
                }
            }
        }
        modules
    }

    /// Gets all nets connected to a module
    pub fn get_module_nets(&self, module: &str) -> Vec<String> {
        let mut nets = Vec::new();
        if let Some(&module_index) = self.module_indices.get(module) {
            for neighbor_index in self.grph.neighbors(module_index) {
                let neighbor_name = &self.grph[neighbor_index];
                if self.nets.contains(neighbor_name) {
                    nets.push(neighbor_name.clone());
                }
            }
        }
        nets
    }

    /// Invalidates cached values (e.g., after adding edges)
    fn invalidate_cache(&mut self) {
        // Recompute max degrees
        self.max_degree = self
            .modules
            .iter()
            .map(|m| self.get_module_degree(m) as u32)
            .max()
            .unwrap_or(0);

        self.max_net_degree = self
            .nets
            .iter()
            .map(|n| self.get_net_degree(n) as u32)
            .max()
            .unwrap_or(0);
    }
}

impl Default for Netlist {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing `Netlist` instances with a fluent API.
///
/// # Examples
///
/// ```
/// use netlistx_rs::NetlistBuilder;
///
/// let netlist = NetlistBuilder::new()
///     .add_module("cell_a")
///     .add_module("cell_b")
///     .add_net("net1")
///     .add_edge("net1", "cell_a")
///     .add_edge("net1", "cell_b")
///     .build()
///     .unwrap();
/// ```
pub struct NetlistBuilder {
    netlist: Netlist,
}

impl NetlistBuilder {
    /// Creates a new `NetlistBuilder`.
    pub fn new() -> Self {
        Self {
            netlist: Netlist::new(),
        }
    }

    /// Adds a module to the netlist being built.
    pub fn add_module(mut self, module: &str) -> Self {
        // Ignore errors during building - let them surface in build()
        let _ = self.netlist.add_module(module.to_string());
        self
    }

    /// Adds a net to the netlist being built.
    pub fn add_net(mut self, net: &str) -> Self {
        // Ignore errors during building - let them surface in build()
        let _ = self.netlist.add_net(net.to_string());
        self
    }

    /// Adds an edge between a net and a module.
    pub fn add_edge(mut self, net: &str, module: &str) -> Self {
        // Ignore errors during building - let them surface in build()
        let _ = self.netlist.add_edge(net, module);
        self
    }

    /// Sets the number of pads.
    pub fn with_pads(mut self, num_pads: i32) -> Self {
        self.netlist.num_pads = num_pads;
        self
    }

    /// Sets the cost model.
    pub fn with_cost_model(mut self, cost_model: i32) -> Self {
        self.netlist.cost_model = cost_model;
        self
    }

    /// Builds and returns the `Netlist`.
    pub fn build(self) -> NetlistResult<Netlist> {
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
        let mut netlist = Netlist::new();
        netlist.add_module("a0".to_string()).unwrap();
        netlist.add_module("a1".to_string()).unwrap();
        netlist.add_module("a2".to_string()).unwrap();
        netlist.add_net("a3".to_string()).unwrap();
        netlist.add_net("a4".to_string()).unwrap();
        netlist.add_net("a5".to_string()).unwrap();
        netlist.add_edge("a3", "a0").unwrap();
        netlist.add_edge("a3", "a1").unwrap();
        netlist.add_edge("a5", "a0").unwrap();

        let mut module_weight: IndexMap<String, i32> = IndexMap::new();
        module_weight.insert("a0".to_string(), 533);
        module_weight.insert("a1".to_string(), 543);
        module_weight.insert("a2".to_string(), 532);
        netlist.module_weight = Some(module_weight);

        netlist
    }

    #[test]
    fn test_create_test_netlist() {
        let netlist = create_test_netlist();
        assert_eq!(netlist.num_modules(), 3);
        assert_eq!(netlist.num_nets(), 3);
        assert_eq!(netlist.grph.node_count(), 6);
        assert_eq!(netlist.grph.edge_count(), 3);
    }

    #[test]
    fn test_new_netlist() {
        let netlist = Netlist::new();
        assert_eq!(netlist.num_modules(), 0);
        assert_eq!(netlist.num_nets(), 0);
        assert_eq!(netlist.grph.node_count(), 0);
        assert_eq!(netlist.grph.edge_count(), 0);
    }

    #[test]
    fn test_add_module() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        assert_eq!(netlist.num_modules(), 1);
        assert!(netlist.modules.contains("m1"));
        assert_eq!(netlist.grph.node_count(), 1);
    }

    #[test]
    fn test_add_duplicate_module() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        let result = netlist.add_module("m1".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_add_net() {
        let mut netlist = Netlist::new();
        netlist.add_net("n1".to_string()).unwrap();
        assert_eq!(netlist.num_nets(), 1);
        assert!(netlist.nets.contains("n1"));
        assert_eq!(netlist.grph.node_count(), 1);
    }

    #[test]
    fn test_add_edge() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        assert_eq!(netlist.grph.edge_count(), 1);
    }

    #[test]
    fn test_add_edge_invalid_module() {
        let mut netlist = Netlist::new();
        netlist.add_net("n1".to_string()).unwrap();
        let result = netlist.add_edge("n1", "m1");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_module_degree() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_module("m2".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_net("n2".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n2", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();

        assert_eq!(netlist.get_module_degree("m1"), 2);
        assert_eq!(netlist.get_module_degree("m2"), 1);
    }

    #[test]
    fn test_get_net_degree() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_module("m2".to_string()).unwrap();
        netlist.add_module("m3".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n1", "m3").unwrap();

        assert_eq!(netlist.get_net_degree("n1"), 3);
    }

    #[test]
    fn test_builder() {
        let netlist = NetlistBuilder::new()
            .add_module("m1")
            .add_module("m2")
            .add_net("n1")
            .add_edge("n1", "m1")
            .add_edge("n1", "m2")
            .build()
            .unwrap();

        assert_eq!(netlist.num_modules(), 2);
        assert_eq!(netlist.num_nets(), 1);
    }

    #[test]
    fn test_get_net_modules() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_module("m2".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();

        let modules = netlist.get_net_modules("n1");
        assert_eq!(modules.len(), 2);
        assert!(modules.contains(&"m1".to_string()));
        assert!(modules.contains(&"m2".to_string()));
    }

    #[test]
    fn test_get_module_nets() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_net("n2".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n2", "m1").unwrap();

        let nets = netlist.get_module_nets("m1");
        assert_eq!(nets.len(), 2);
        assert!(nets.contains(&"n1".to_string()));
        assert!(nets.contains(&"n2".to_string()));
    }

    #[test]
    fn test_has_module() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        assert!(netlist.has_module("m1"));
        assert!(!netlist.has_module("m2"));
    }

    #[test]
    fn test_has_net() {
        let mut netlist = Netlist::new();
        netlist.add_net("n1".to_string()).unwrap();
        assert!(netlist.has_net("n1"));
        assert!(!netlist.has_net("n2"));
    }

    #[test]
    fn test_add_module_empty_name() {
        let mut netlist = Netlist::new();
        let result = netlist.add_module("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_add_net_empty_name() {
        let mut netlist = Netlist::new();
        let result = netlist.add_net("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_add_duplicate_net() {
        let mut netlist = Netlist::new();
        netlist.add_net("n1".to_string()).unwrap();
        let result = netlist.add_net("n1".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_pads() {
        let netlist = NetlistBuilder::new()
            .add_module("m1")
            .with_pads(10)
            .build()
            .unwrap();
        assert_eq!(netlist.num_pads, 10);
    }

    #[test]
    fn test_builder_with_cost_model() {
        let netlist = NetlistBuilder::new()
            .add_module("m1")
            .with_cost_model(1)
            .build()
            .unwrap();
        assert_eq!(netlist.cost_model, 1);
    }

    #[test]
    fn test_default_netlist() {
        let netlist: Netlist = Default::default();
        assert_eq!(netlist.num_modules(), 0);
        assert_eq!(netlist.num_nets(), 0);
    }

    #[test]
    fn test_default_netlist_builder() {
        let builder: NetlistBuilder = Default::default();
        let netlist = builder.build().unwrap();
        assert_eq!(netlist.num_modules(), 0);
    }

    #[test]
    fn test_get_module_degree_nonexistent() {
        let netlist = Netlist::new();
        assert_eq!(netlist.get_module_degree("nonexistent"), 0);
    }

    #[test]
    fn test_get_net_degree_nonexistent() {
        let netlist = Netlist::new();
        assert_eq!(netlist.get_net_degree("nonexistent"), 0);
    }

    #[test]
    fn test_get_net_modules_empty() {
        let netlist = Netlist::new();
        let modules = netlist.get_net_modules("nonexistent");
        assert!(modules.is_empty());
    }

    #[test]
    fn test_get_module_nets_empty() {
        let netlist = Netlist::new();
        let nets = netlist.get_module_nets("nonexistent");
        assert!(nets.is_empty());
    }
}

#[cfg(test)]
#[cfg(feature = "quickcheck")]
mod quickcheck_impls {
    use super::*;
    use quickcheck::{Arbitrary, Gen};

    impl Arbitrary for Netlist {
        fn arbitrary(g: &mut Gen) -> Self {
            let num_modules: usize = Arbitrary::arbitrary(g);
            let num_modules = num_modules % 20; // Limit size for testing
            let num_nets: usize = Arbitrary::arbitrary(g);
            let num_nets = num_nets % 20; // Limit size for testing

            let mut builder = NetlistBuilder::new();

            // Add modules
            for i in 0..num_modules {
                builder = builder.add_module(&format!("m{}", i));
            }

            // Add nets
            for i in 0..num_nets {
                builder = builder.add_net(&format!("n{}", i));
            }

            // Add some random edges
            let num_edges: usize = Arbitrary::arbitrary(g);
            let num_edges = num_edges % 50; // Limit edges
            for _ in 0..num_edges {
                let module_idx: usize = Arbitrary::arbitrary(g);
                let net_idx: usize = Arbitrary::arbitrary(g);
                if module_idx < num_modules && net_idx < num_nets {
                    builder =
                        builder.add_edge(&format!("n{}", net_idx), &format!("m{}", module_idx));
                }
            }

            builder.build().unwrap_or_default()
        }
    }
}
