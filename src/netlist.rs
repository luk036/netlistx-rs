use petgraph::Graph;
use std::collections::{HashMap, HashSet};

/// A struct representing a netlist, which is a graph-like data structure used in electronic design automation.
///
/// The `Netlist` struct contains information about the modules and nets in the netlist, as well as the underlying graph
/// representation and various metadata such as weights and fixed modules.
#[derive(Debug, Clone)]
pub struct Netlist {
    pub num_pads: i32,
    pub cost_model: i32,
    pub grph: Graph<String, ()>,
    pub modules: Vec<String>,
    pub nets: Vec<String>,
    pub num_modules: usize,
    pub num_nets: usize,
    pub net_weight: Option<HashMap<String, i32>>,
    pub module_weight: Option<HashMap<String, i32>>,
    pub module_fixed: HashSet<String>,
    pub max_degree: u32,
    pub max_net_degree: u32,
}

impl Netlist {
    /// Creates a new, empty `Netlist`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::netlist::Netlist;
    ///
    /// let netlist = Netlist::new();
    /// assert_eq!(netlist.num_modules, 0);
    /// assert_eq!(netlist.num_nets, 0);
    /// ```
    pub fn new() -> Self {
        Netlist {
            num_pads: 0,
            cost_model: 0,
            grph: Graph::new(),
            modules: Vec::new(),
            nets: Vec::new(),
            num_modules: 0,
            num_nets: 0,
            net_weight: None,
            module_weight: None,
            module_fixed: HashSet::new(),
            max_degree: 0,
            max_net_degree: 0,
        }
    }

    /// Adds a module to the netlist.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::netlist::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_module("m1".to_string());
    /// assert_eq!(netlist.num_modules, 1);
    /// ```
    pub fn add_module(&mut self, module: String) {
        self.modules.push(module.clone());
        self.grph.add_node(module);
        self.num_modules = self.modules.len();
    }

    /// Adds a net to the netlist.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::netlist::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_net("n1".to_string());
    /// assert_eq!(netlist.num_nets, 1);
    /// ```
    pub fn add_net(&mut self, net: String) {
        self.nets.push(net.clone());
        self.grph.add_node(net);
        self.num_nets = self.nets.len();
    }

    /// Adds an edge between a net and a module.
    ///
    /// # Examples
    ///
    /// ```
    /// use netlistx_rs::netlist::Netlist;
    ///
    /// let mut netlist = Netlist::new();
    /// netlist.add_module("m1".to_string());
    /// netlist.add_net("n1".to_string());
    /// netlist.add_edge("n1", "m1");
    /// assert_eq!(netlist.grph.edge_count(), 1);
    /// ```
    pub fn add_edge(&mut self, net: &str, module: &str) {
        let net_index = self.grph.node_indices().find(|i| self.grph[*i] == net);
        let module_index = self.grph.node_indices().find(|i| self.grph[*i] == module);
        if let (Some(net_index), Some(module_index)) = (net_index, module_index) {
            self.grph.add_edge(net_index, module_index, ());
        }
    }
}

impl Default for Netlist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a test netlist for use in unit tests or other purposes.
    ///
    /// This function creates a simple netlist with a few modules and nets, and returns a `Netlist` struct
    /// that represents this netlist. The netlist has the following properties:
    ///
    /// - 6 modules: "a0", "a1", "a2", "a3", "a4", and "a5"
    /// - 3 nets: "a3", "a4", and "a5"
    /// - Module weights for "a0", "a1", and "a2" are 533, 543, and 532 respectively
    /// - No net weights are set
    /// - No fixed modules
    /// - Maximum degree and maximum net degree are both 0
    ///
    /// This function is primarily intended for use in unit tests, where a simple, known netlist is needed
    /// for testing purposes.
    fn create_test_netlist() -> Netlist {
        let mut grph = Graph::new();
        let a0 = grph.add_node("a0".to_string());
        let a1 = grph.add_node("a1".to_string());
        let _a2 = grph.add_node("a2".to_string());
        let a3 = grph.add_node("a3".to_string());
        let _a4 = grph.add_node("a4".to_string());
        let a5 = grph.add_node("a5".to_string());
        let module_weight: HashMap<String, i32> = [
            ("a0".to_string(), 533),
            ("a1".to_string(), 543),
            ("a2".to_string(), 532),
        ]
        .iter()
        .cloned()
        .collect();
        grph.extend_with_edges([(a3, a0), (a3, a1), (a5, a0)]);
        let modules = vec!["a0".to_string(), "a1".to_string(), "a2".to_string()];
        let nets = vec!["a3".to_string(), "a4".to_string(), "a5".to_string()];
        let mut hyprgraph = Netlist {
            num_pads: 0,
            cost_model: 0,
            grph,
            modules: modules.clone(),
            nets: nets.clone(),
            num_modules: modules.len(),
            num_nets: nets.len(),
            net_weight: None,
            module_weight: None,
            module_fixed: HashSet::new(),
            max_degree: 0,
            max_net_degree: 0,
        };
        hyprgraph.module_weight = Some(module_weight);
        hyprgraph
    }

    #[test]
    fn test_create_test_netlist() {
        let netlist = create_test_netlist();
        assert_eq!(netlist.num_modules, 3);
        assert_eq!(netlist.num_nets, 3);
        assert_eq!(netlist.grph.node_count(), 6);
        assert_eq!(netlist.grph.edge_count(), 3);
    }

    #[test]
    fn test_new_netlist() {
        let netlist = Netlist::new();
        assert_eq!(netlist.num_modules, 0);
        assert_eq!(netlist.num_nets, 0);
        assert_eq!(netlist.grph.node_count(), 0);
        assert_eq!(netlist.grph.edge_count(), 0);
    }

    #[test]
    fn test_add_module() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string());
        assert_eq!(netlist.num_modules, 1);
        assert_eq!(netlist.modules, vec!["m1".to_string()]);
        assert_eq!(netlist.grph.node_count(), 1);
    }

    #[test]
    fn test_add_net() {
        let mut netlist = Netlist::new();
        netlist.add_net("n1".to_string());
        assert_eq!(netlist.num_nets, 1);
        assert_eq!(netlist.nets, vec!["n1".to_string()]);
        assert_eq!(netlist.grph.node_count(), 1);
    }

    #[test]
    fn test_add_edge() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string());
        netlist.add_net("n1".to_string());
        netlist.add_edge("n1", "m1");
        assert_eq!(netlist.grph.edge_count(), 1);
    }

    #[test]
    fn test_default_netlist() {
        let netlist: Netlist = Default::default();
        assert_eq!(netlist.num_modules, 0);
        assert_eq!(netlist.num_nets, 0);
        assert_eq!(netlist.grph.node_count(), 0);
        assert_eq!(netlist.grph.edge_count(), 0);
    }
}
