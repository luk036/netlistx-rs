use petgraph::Graph;
use std::collections::{HashMap, HashSet};

/// A struct representing a netlist, which is a graph-like data structure used in electronic design automation.
/// 
/// The `Netlist` struct contains information about the modules and nets in the netlist, as well as the underlying graph
/// representation and various metadata such as weights and fixed modules.
///
/// The `'a` lifetime parameter is used to ensure that the references to module and net names in the `grph`, `modules`, and `nets`
/// fields have the same lifetime as the `Netlist` struct itself.
struct Netlist<'a> {
    num_pads: i32,
    cost_model: i32,
    grph: Graph<&'a str, ()>,
    modules: Vec<&'a str>,
    nets: Vec<&'a str>,
    num_modules: usize,
    num_nets: usize,
    net_weight: Option<HashMap<&'a str, i32>>,
    module_weight: Option<HashMap<&'a str, i32>>,
    module_fixed: HashSet<&'a str>,
    max_degree: u32,
    max_net_degree: u32,
}

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
fn create_test_netlist<'a>() -> Netlist<'a> {
    let mut grph = Graph::new();
    let a0 = grph.add_node("a0");
    let a1 = grph.add_node("a1");
    let a2 = grph.add_node("a2");
    let a3 = grph.add_node("a3");
    let a4 = grph.add_node("a4");
    let a5 = grph.add_node("a5");
    let module_weight: HashMap<&str, i32> = [("a0", 533), ("a1", 543), ("a2", 532)]
        .iter()
        .cloned()
        .collect();
    grph.extend_with_edges(&[(a3, a0), (a3, a1), (a5, a0)]);
    grph.graph_mut().set_node_count(6);
    grph.graph_mut().set_edge_count(3);
    let modules = vec!["a0", "a1", "a2"];
    let nets = vec!["a3", "a4", "a5"];
    let mut hyprgraph = Netlist {
        num_pads: 0,
        cost_model: 0,
        grph,
        modules,
        nets,
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
