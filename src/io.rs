use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::netlist::Netlist;

/// Error type for I/O operations
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Result type for I/O operations
pub type IoResult<T> = Result<T, IoError>;

/// Input file format for netlist reading
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    HMetis,
    Json,
    Dimacs,
    NetD,
    AutoDetect,
}

/// Output file format for netlist/partition writing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    HMetis,
    Json,
}

/// Detect input format from file extension
pub fn detect_input_format(filename: &str) -> InputFormat {
    if filename.ends_with(".net") || filename.ends_with(".netD") {
        return InputFormat::NetD;
    }
    if filename.ends_with(".hgr") || filename.ends_with(".graph") {
        return InputFormat::HMetis;
    }
    if filename.ends_with(".json") {
        return InputFormat::Json;
    }
    if filename.ends_with(".dimacs") {
        return InputFormat::Dimacs;
    }
    InputFormat::AutoDetect
}

/// Read a netlist from a file, auto-detecting format.
pub fn read_netlist<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let path = path.as_ref();
    let filename = path
        .to_str()
        .ok_or_else(|| IoError::InvalidFormat("Non-UTF-8 path".to_string()))?;
    let format = detect_input_format(filename);
    read_hypergraph(path, format)
}

/// Read a netlist in the specified format.
pub fn read_hypergraph<P: AsRef<Path>>(path: P, format: InputFormat) -> IoResult<Netlist> {
    let actual_format = if format == InputFormat::AutoDetect {
        let filename = path
            .as_ref()
            .to_str()
            .ok_or_else(|| IoError::InvalidFormat("Non-UTF-8 path".to_string()))?;
        detect_input_format(filename)
    } else {
        format
    };

    match actual_format {
        InputFormat::HMetis => read_hmetis_format(path),
        InputFormat::Json => read_json_format(path),
        InputFormat::Dimacs => read_dimacs_format(path),
        InputFormat::NetD | InputFormat::AutoDetect => read_netd_format(path),
    }
}

/// Read a netlist in IBM netD format.
///
/// Ported from C++ `read_netD()` / `read_netD_format()` in `readwrite.cpp`.
fn read_netd_format<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let content = std::fs::read_to_string(path.as_ref()).map_err(IoError::IoError)?;
    let mut lines = content.lines();

    let header_line = lines.next().ok_or_else(|| IoError::ParseError {
        line: 1,
        message: "Empty file".to_string(),
    })?;

    let header_parts: Vec<&str> = header_line.split_whitespace().collect();
    if header_parts.len() < 4 {
        return Err(IoError::ParseError {
            line: 1,
            message: "Invalid netD header: expected 4 numbers".to_string(),
        });
    }

    let num_pins: u32 = header_parts[1].parse().map_err(|_| IoError::ParseError {
        line: 1,
        message: "Invalid numPins".to_string(),
    })?;
    let _num_nets: u32 = header_parts[2].parse().map_err(|_| IoError::ParseError {
        line: 1,
        message: "Invalid numNets".to_string(),
    })?;
    let num_modules: u32 = header_parts[3].parse().map_err(|_| IoError::ParseError {
        line: 1,
        message: "Invalid numModules".to_string(),
    })?;
    let pad_offset: u32 = if header_parts.len() > 4 {
        header_parts[4].parse().unwrap_or(0)
    } else {
        0
    };

    let mut netlist = Netlist::new();
    for i in 0..num_modules {
        netlist
            .add_module(format!("m{}", i))
            .map_err(|e| IoError::ParseError {
                line: 0,
                message: e.to_string(),
            })?;
    }

    let mut edge_idx = num_modules;
    let mut pin_count = 0;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        if pin_count >= num_pins {
            break;
        }

        let chars: Vec<char> = line.trim().chars().collect();
        if chars.is_empty() {
            continue;
        }

        let mut pos = 0;

        let node: u32 = if chars[pos] == 'a' {
            pos += 1;
            let num_str: String = chars[pos..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            pos += num_str.len();
            num_str.parse().unwrap_or(0)
        } else if chars[pos] == 'p' {
            pos += 1;
            let num_str: String = chars[pos..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            pos += num_str.len();
            let n: u32 = num_str.parse().unwrap_or(0);
            n + pad_offset
        } else {
            pin_count += 1;
            continue;
        };

        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos < chars.len() && chars[pos] == 's' {
            edge_idx += 1;
        }

        let net_name = format!("n{}", edge_idx - 1 - num_modules);
        if !netlist.has_net(&net_name) {
            let _ = netlist.add_net(net_name.clone());
        }

        let mod_name = format!("m{}", node);
        let _ = netlist.add_edge(&net_name, &mod_name);

        pin_count += 1;
    }

    netlist.num_pads = (num_modules - pad_offset - 1) as i32;
    Ok(netlist)
}

/// Read IBM .are file with module weights.
///
/// Ported from C++ `readAre()` in `readwrite.cpp`.
pub fn read_are<P: AsRef<Path>>(netlist: &mut Netlist, path: P) -> IoResult<()> {
    let content = std::fs::read_to_string(path.as_ref()).map_err(IoError::IoError)?;

    let pad_offset = netlist.num_modules() as u32 - netlist.num_pads as u32 - 1;
    let num_modules = netlist.num_modules();

    let mut module_weights: Vec<u32> = vec![1; num_modules];

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let chars: Vec<char> = trimmed.chars().collect();
        if chars.is_empty() {
            continue;
        }

        let node: u32 = if chars[0] == 'a' {
            let num_str: String = chars[1..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            num_str.parse().unwrap_or(0)
        } else if chars[0] == 'p' {
            let num_str: String = chars[1..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            let n: u32 = num_str.parse().unwrap_or(0);
            n + pad_offset
        } else {
            continue;
        };

        let rest: String = chars
            .iter()
            .skip_while(|c| *c != &' ' && *c != &'\t')
            .skip_while(|c| c.is_whitespace())
            .collect();
        let weight_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(w) = weight_str.parse::<u32>() {
            if (node as usize) < num_modules {
                module_weights[node as usize] = w;
            }
        }
    }

    for (i, w) in module_weights.iter().enumerate() {
        if i < num_modules {
            let mod_name = format!("m{}", i);
            if netlist.has_module(&mod_name) {
                netlist.set_module_weight(&mod_name, *w as i32);
            }
        }
    }

    Ok(())
}

/// Read a netlist in hMetis format.
///
/// Ported from C++ `read_hmetis_format()` in `readwrite.cpp`.
fn read_hmetis_format<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let header = lines
        .next()
        .ok_or_else(|| IoError::ParseError {
            line: 1,
            message: "Empty file".to_string(),
        })??
        .trim()
        .to_string();

    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(IoError::ParseError {
            line: 1,
            message: "Invalid hMetis header: expected at least 2 numbers".to_string(),
        });
    }

    let num_nets: usize = parts[0].parse().map_err(|_| IoError::ParseError {
        line: 1,
        message: "Invalid numNets".to_string(),
    })?;
    let num_vertices: usize = parts[1].parse().map_err(|_| IoError::ParseError {
        line: 1,
        message: "Invalid numVertices".to_string(),
    })?;

    let mut netlist = Netlist::new();
    for i in 0..num_vertices {
        netlist
            .add_module(format!("m{}", i))
            .map_err(|e| IoError::ParseError {
                line: 0,
                message: e.to_string(),
            })?;
    }

    let mut net_idx = 0usize;
    for line in lines {
        if net_idx >= num_nets {
            break;
        }
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('c') {
            continue;
        }

        let net_name = format!("n{}", net_idx);
        let _ = netlist.add_net(net_name.clone());

        for token in trimmed.split_whitespace() {
            if let Ok(v) = token.parse::<usize>() {
                let v_idx = if v > 0 { v - 1 } else { v };
                if v_idx < num_vertices {
                    let mod_name = format!("m{}", v_idx);
                    let _ = netlist.add_edge(&net_name, &mod_name);
                }
            }
        }

        net_idx += 1;
    }

    Ok(netlist)
}

/// Read a netlist in DIMACS format.
///
/// Ported from C++ `read_dimacs_format()` in `readwrite.cpp`.
fn read_dimacs_format<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let content = std::fs::read_to_string(path.as_ref())?;

    let mut num_vertices: usize = 0;
    let mut num_nets_out: usize = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('c') || trimmed.starts_with('e') {
            continue;
        }
        if trimmed.starts_with('p') {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                num_vertices = parts[2].parse().unwrap_or(0);
                num_nets_out = parts[3].parse().unwrap_or(0);
            }
            break;
        }
    }

    let mut netlist = Netlist::new();
    for i in 0..num_vertices {
        let _ = netlist.add_module(format!("m{}", i));
    }
    for i in 0..num_nets_out {
        let _ = netlist.add_net(format!("n{}", i));
    }

    Ok(netlist)
}

/// Read a netlist from a Yosys JSON file.
///
/// Yosys JSON format contains modules with cells (gates) and ports (I/O).
/// Cells become modules, ports become fixed modules with weight 0.
/// Nets are identified by integer wire IDs from the Yosys representation.
///
/// Ported from Python `read_yosys_json()` in `netlist.py`.
pub fn read_yosys_json<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);
    let data: serde_json::Value = serde_json::from_reader(reader)?;

    let modules = data
        .get("modules")
        .ok_or_else(|| IoError::InvalidFormat("Missing 'modules' key".to_string()))?;

    let module_name = modules
        .as_object()
        .and_then(|m| m.keys().next())
        .ok_or_else(|| IoError::InvalidFormat("Empty modules".to_string()))?
        .clone();

    let module_data = &modules[&module_name];

    let cells = module_data
        .get("cells")
        .ok_or_else(|| IoError::InvalidFormat("Missing 'cells'".to_string()))?;
    let ports = module_data
        .get("ports")
        .ok_or_else(|| IoError::InvalidFormat("Missing 'ports'".to_string()))?;

    // 1. Collect all cells and ports
    let cell_names: Vec<String> = cells
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let port_names: Vec<String> = ports
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let _num_cells = cell_names.len();
    let num_ports = port_names.len();

    // 2. Collect all unique net IDs (must be integers, skip strings like "0" constants)
    let mut all_nets_set: HashSet<u32> = HashSet::new();

    if let Some(ports_obj) = ports.as_object() {
        for port_info in ports_obj.values() {
            if let Some(bits) = port_info.get("bits").and_then(|b| b.as_array()) {
                for bit in bits {
                    if let Some(n) = bit.as_u64() {
                        all_nets_set.insert(n as u32);
                    }
                }
            }
        }
    }

    if let Some(netnames) = module_data.get("netnames") {
        if let Some(obj) = netnames.as_object() {
            for netinfo in obj.values() {
                if let Some(bits) = netinfo.get("bits").and_then(|b| b.as_array()) {
                    for bit in bits {
                        if let Some(n) = bit.as_u64() {
                            all_nets_set.insert(n as u32);
                        }
                    }
                }
            }
        }
    }

    if let Some(cells_obj) = cells.as_object() {
        for cell_info in cells_obj.values() {
            if let Some(connections) = cell_info.get("connections").and_then(|c| c.as_object()) {
                for conn in connections.values() {
                    if let Some(arr) = conn.as_array() {
                        for net_id in arr {
                            // Only add integer net IDs (skip string constants)
                            if let Some(n) = net_id.as_u64() {
                                all_nets_set.insert(n as u32);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut nets_list: Vec<u32> = all_nets_set.into_iter().collect();
    nets_list.sort();

    // 3. Build the netlist
    let mut netlist = Netlist::new();

    // Add cells as modules (use original cell names)
    for cell_name in &cell_names {
        netlist
            .add_module(cell_name.clone())
            .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
    }

    // Add ports as modules with "PORT_" prefix
    for port_name in &port_names {
        let port_mod = format!("PORT_{}", port_name);
        netlist
            .add_module(port_mod)
            .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
    }

    // Add nets (wire IDs as string names)
    for net_id in &nets_list {
        let net_name = net_id.to_string();
        netlist
            .add_net(net_name)
            .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
    }

    // 4. Add edges: cell connections
    //    In the Python version: cells use original names, edges connect cell <-> net
    if let Some(cells_obj) = cells.as_object() {
        for (cell_name, cell_data) in cells_obj.iter() {
            if let Some(connections) = cell_data.get("connections").and_then(|c| c.as_object()) {
                for conn in connections.values() {
                    if let Some(arr) = conn.as_array() {
                        for net_id in arr {
                            if let Some(n) = net_id.as_u64() {
                                let n = n as u32;
                                if nets_list.contains(&n) {
                                    let net_name = n.to_string();
                                    let _ = netlist.add_edge(&net_name, cell_name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Add edges: port connections
    //    Each port connects to its bits.
    if let Some(ports_obj) = ports.as_object() {
        for (port_name, port_info) in ports_obj.iter() {
            if let Some(bits) = port_info.get("bits").and_then(|b| b.as_array()) {
                for bit in bits {
                    if let Some(n) = bit.as_u64() {
                        let n = n as u32;
                        if nets_list.contains(&n) {
                            let net_name = n.to_string();
                            let port_mod = format!("PORT_{}", port_name);
                            let _ = netlist.add_edge(&net_name, &port_mod);
                        }
                    }
                }
            }
        }
    }

    // 6. Set metadata
    netlist.num_pads = num_ports as i32;

    // Set module weights: cells = 1, ports = 0
    for cell_name in &cell_names {
        netlist.set_module_weight(cell_name, 1);
    }
    for port_name in &port_names {
        let port_mod = format!("PORT_{}", port_name);
        netlist.set_module_weight(&port_mod, 0);
    }

    // Mark ports as fixed
    for port_name in &port_names {
        netlist
            .module_fixed
            .insert(format!("PORT_{}", port_name));
    }
    netlist.has_fixed_modules = num_ports > 0;

    Ok(netlist)
}

/// Read a netlist from standard node-link JSON format (as written by `write_json`).
///
/// The JSON file must have a "graph" object with "num_modules" and "num_nets",
/// a "nodes" array, and edges in either "links" or "edges" arrays.
///
/// Ported from Python `read_json()` in `netlist.py`.
pub fn read_node_link_json<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);
    let data: serde_json::Value = serde_json::from_reader(reader)?;

    let graph_obj = data
        .get("graph")
        .ok_or_else(|| IoError::InvalidFormat("Missing 'graph' key".to_string()))?;

    let num_modules = graph_obj
        .get("num_modules")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| IoError::InvalidFormat("Missing num_modules".to_string()))? as usize;
    let _num_nets = graph_obj
        .get("num_nets")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| IoError::InvalidFormat("Missing num_nets".to_string()))? as usize;
    let num_pads = graph_obj
        .get("num_pads")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as i32;

    let nodes = data
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| IoError::InvalidFormat("Missing 'nodes' array".to_string()))?;

    // First num_modules entries are modules, rest are nets
    let mut netlist = Netlist::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = node
            .get("id")
            .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
            .ok_or_else(|| IoError::InvalidFormat(format!("Node {} missing valid 'id'", i)))?;
        if i < num_modules {
            netlist
                .add_module(id)
                .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
        } else {
            netlist
                .add_net(id)
                .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
        }
    }

    // Add edges (from "links" or "edges")
    let edges = data
        .get("links")
        .or_else(|| data.get("edges"))
        .and_then(|v| v.as_array());

    if let Some(edges_array) = edges {
        for edge in edges_array {
            let source = edge
                .get("source")
                .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
                .ok_or_else(|| IoError::InvalidFormat("Edge missing valid 'source'".to_string()))?;
            let target = edge
                .get("target")
                .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
                .ok_or_else(|| IoError::InvalidFormat("Edge missing valid 'target'".to_string()))?;
            // In node-link format, we don't know which direction the edge goes.
            // Try both (net, module) and (module, net) orders.
            if netlist.add_edge(&source, &target).is_err() {
                let _ = netlist.add_edge(&target, &source);
            }
        }
    }

    netlist.num_pads = num_pads;

    // Set module weights from node attributes
    for (i, node) in nodes.iter().enumerate() {
        if i < num_modules {
            if let Some(id) = node.get("id").and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string()))) {
                if let Some(w) = node.get("weight").and_then(|v| v.as_i64()) {
                    netlist.set_module_weight(&id, w as i32);
                }
            }
        }
    }

    Ok(netlist)
}

/// Read JSON format: tries Yosys JSON first, falls back to node-link JSON.
fn read_json_format<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    // Read the file content first
    let content = std::fs::read_to_string(path.as_ref())?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    // Detect format: has "modules" key → Yosys, otherwise → node-link
    if data.get("modules").is_some() {
        // Re-parse as Yosys format
        drop(data);
        read_yosys_json(path)
    } else {
        // Node-link format
        drop(data);
        read_node_link_json(path)
    }
}

/// Write a netlist to JSON format.
///
/// Ported from C++ `writeJSON()` in `readwrite.cpp`.
pub fn write_json<P: AsRef<Path>>(netlist: &Netlist, path: P) -> IoResult<()> {
    let mut file = File::create(path.as_ref())?;

    writeln!(file, "{{")?;
    writeln!(file, " \"directed\": false,")?;
    writeln!(file, " \"multigraph\": false,")?;
    writeln!(file, " \"graph\": {{")?;
    writeln!(file, "  \"num_modules\": {},", netlist.num_modules())?;
    writeln!(file, "  \"num_nets\": {},", netlist.num_nets())?;
    writeln!(file, "  \"num_pads\": {}", netlist.num_pads)?;
    writeln!(file, " }},")?;

    writeln!(file, " \"nodes\": [")?;
    for module in &netlist.modules {
        writeln!(file, "  {{ \"id\": \"{}\" }},", module)?;
    }
    writeln!(file, " ],")?;

    writeln!(file, " \"links\": [")?;
    for module in &netlist.modules {
        for net_name in &netlist.get_module_nets(module) {
            writeln!(file, "  {{")?;
            writeln!(file, "   \"source\": \"{}\",", module)?;
            writeln!(file, "   \"target\": \"{}\"", net_name)?;
            writeln!(file, "  }},")?;
        }
    }
    writeln!(file, " ]")?;
    writeln!(file, "}}")?;

    Ok(())
}

/// Write a netlist to a simple text format.
pub fn write_netlist<P: AsRef<Path>>(netlist: &Netlist, path: P) -> IoResult<()> {
    let mut file = File::create(path.as_ref())?;

    writeln!(file, "# Netlist generated by netlistx-rs")?;
    writeln!(file, "# Modules: {}", netlist.num_modules())?;
    writeln!(file, "# Nets: {}", netlist.num_nets())?;
    writeln!(file)?;

    writeln!(file, "# Modules")?;
    for module in &netlist.modules {
        writeln!(file, "MODULE {}", module)?;
    }
    writeln!(file)?;

    writeln!(file, "# Nets")?;
    for net in &netlist.nets {
        let modules = netlist.get_net_modules(net);
        if !modules.is_empty() {
            writeln!(file, "NET {} {}", net, modules.join(" "))?;
        }
    }

    Ok(())
}

/// Write partition in hMetis format (one value per line).
pub fn write_hmetis_partition<W: Write>(part: &[u8], writer: &mut W) -> std::io::Result<()> {
    for &p in part {
        writeln!(writer, "{}", p)?;
    }
    Ok(())
}

/// Write partition in JSON format.
pub fn write_json_partition<W: Write>(part: &[u8], writer: &mut W) -> std::io::Result<()> {
    let values: Vec<String> = part.iter().map(|&p| p.to_string()).collect();
    writeln!(writer, "[{}]", values.join(", "))?;
    Ok(())
}

/// Write partition in the specified format.
pub fn write_partition<W: Write>(
    part: &[u8],
    writer: &mut W,
    format: OutputFormat,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Json => write_json_partition(part, writer),
        OutputFormat::HMetis => write_hmetis_partition(part, writer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_and_read_json() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_module("m2".to_string()).unwrap();
        netlist.add_module("m3".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_net("n2".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        write_json(&netlist, temp_file.path()).unwrap();
        let _ = read_netlist(temp_file.path());
    }

    #[test]
    fn test_write_netlist() {
        let mut netlist = Netlist::new();
        netlist.add_module("m1".to_string()).unwrap();
        netlist.add_module("m2".to_string()).unwrap();
        netlist.add_module("m3".to_string()).unwrap();
        netlist.add_net("n1".to_string()).unwrap();
        netlist.add_net("n2".to_string()).unwrap();
        netlist.add_edge("n1", "m1").unwrap();
        netlist.add_edge("n1", "m2").unwrap();
        netlist.add_edge("n2", "m2").unwrap();
        netlist.add_edge("n2", "m3").unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        write_netlist(&netlist, temp_file.path()).unwrap();
        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("NET"));
        assert!(content.contains("MODULE"));
    }

    #[test]
    fn test_detect_input_format() {
        assert_eq!(detect_input_format("test.hgr"), InputFormat::HMetis);
        assert_eq!(detect_input_format("test.graph"), InputFormat::HMetis);
        assert_eq!(detect_input_format("test.json"), InputFormat::Json);
        assert_eq!(detect_input_format("test.net"), InputFormat::NetD);
        assert_eq!(detect_input_format("test.dimacs"), InputFormat::Dimacs);
        assert_eq!(detect_input_format("unknown"), InputFormat::AutoDetect);
    }

    #[test]
    fn test_write_hmetis_partition() {
        let part = vec![0u8, 1, 0, 1, 0];
        let mut buf = Vec::new();
        write_hmetis_partition(&part, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "0\n1\n0\n1\n0\n");
    }

    #[test]
    fn test_write_json_partition() {
        let part = vec![0u8, 1, 0, 1, 0];
        let mut buf = Vec::new();
        write_json_partition(&part, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "[0, 1, 0, 1, 0]\n");
    }

    #[test]
    fn test_write_partition() {
        let part = vec![0u8, 1, 0, 1, 0];

        let mut buf1 = Vec::new();
        write_partition(&part, &mut buf1, OutputFormat::HMetis).unwrap();
        assert_eq!(String::from_utf8(buf1).unwrap(), "0\n1\n0\n1\n0\n");

        let mut buf2 = Vec::new();
        write_partition(&part, &mut buf2, OutputFormat::Json).unwrap();
        assert_eq!(String::from_utf8(buf2).unwrap(), "[0, 1, 0, 1, 0]\n");
    }

    #[test]
    fn test_read_invalid_format() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().with_extension("invalid");
        std::fs::rename(temp_file.path(), &temp_path).unwrap();

        let result = read_netlist(&temp_path);
        assert!(result.is_err());
    }

    // --- Yosys JSON tests ---

    fn make_yosys_json(
        cells: serde_json::Value,
        ports: serde_json::Value,
        netnames: Option<serde_json::Value>,
    ) -> tempfile::NamedTempFile {
        let mut data = serde_json::json!({
            "modules": {
                "top": {
                    "cells": cells,
                    "ports": ports,
                }
            }
        });
        if let Some(nn) = netnames {
            data["modules"]["top"]["netnames"] = nn;
        }
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let content = serde_json::to_string(&data).unwrap();
        use std::io::Write;
        write!(tmp, "{}", content).unwrap();
        tmp
    }

    #[test]
    fn test_yosys_simple_and_gate() {
        let cells = serde_json::json!({
            "and1": {
                "type": "$and",
                "connections": {
                    "A": [0],
                    "B": [1],
                    "Y": [2],
                },
            }
        });
        let ports = serde_json::json!({
            "a": {"direction": "input", "bits": [0]},
            "b": {"direction": "input", "bits": [1]},
            "y": {"direction": "output", "bits": [2]},
        });
        let netnames = serde_json::json!({
            "net_a": {"bits": [0]},
            "net_b": {"bits": [1]},
            "net_y": {"bits": [2]},
        });

        let tmp = make_yosys_json(cells, ports, Some(netnames));
        let netlist = read_yosys_json(tmp.path()).unwrap();

        // 1 cell + 3 ports = 4 modules
        assert_eq!(netlist.num_modules(), 4);
        // 3 distinct nets
        assert_eq!(netlist.num_nets(), 3);
        // 4 modules + 3 nets = 7 nodes
        assert_eq!(netlist.number_of_nodes(), 7);
        // Each cell-net connection + each port-net connection = 3 + 3 = 6 pins
        assert_eq!(netlist.grph.edge_count(), 6);
        assert_eq!(netlist.num_pads, 3);

        // Cells have weight 1
        assert_eq!(netlist.get_module_weight("and1"), 1);
        // Ports have weight 0
        assert_eq!(netlist.get_module_weight("PORT_a"), 0);
        assert_eq!(netlist.get_module_weight("PORT_b"), 0);
        assert_eq!(netlist.get_module_weight("PORT_y"), 0);

        // Ports are fixed
        assert!(netlist.module_fixed.contains("PORT_a"));
        assert!(netlist.module_fixed.contains("PORT_b"));
        assert!(netlist.module_fixed.contains("PORT_y"));
        assert!(netlist.has_fixed_modules);
    }

    #[test]
    fn test_yosys_two_cells_shared_net() {
        let cells = serde_json::json!({
            "inv1": {
                "type": "$_INV_",
                "connections": {"A": [0], "Y": [1]},
            },
            "inv2": {
                "type": "$_INV_",
                "connections": {"A": [1], "Y": [2]},
            },
        });
        let ports = serde_json::json!({
            "in": {"direction": "input", "bits": [0]},
            "out": {"direction": "output", "bits": [2]},
        });

        let tmp = make_yosys_json(cells, ports, None);
        let netlist = read_yosys_json(tmp.path()).unwrap();

        // 2 cells + 2 ports = 4 modules
        assert_eq!(netlist.num_modules(), 4);
        // 3 distinct nets (0, 1, 2)
        assert_eq!(netlist.num_nets(), 3);
        assert_eq!(netlist.num_pads, 2);
        // 4 modules + 3 nets = 7 nodes
        assert_eq!(netlist.number_of_nodes(), 7);
    }

    #[test]
    fn test_yosys_ignores_string_constants() {
        let cells = serde_json::json!({
            "and1": {
                "type": "$and",
                "connections": {
                    "A": [0],
                    "B": [1],
                    "Y": [2],
                },
            },
            "const1": {
                "type": "$const",
                "connections": {
                    "Y": [0],
                    "A": ["0", "0", "0", "0"],
                },
            },
        });
        let ports = serde_json::json!({
            "a": {"direction": "input", "bits": [0]},
            "b": {"direction": "input", "bits": [1]},
            "y": {"direction": "output", "bits": [2]},
        });

        let tmp = make_yosys_json(cells, ports, None);
        let netlist = read_yosys_json(tmp.path()).unwrap();

        // 2 cells + 3 ports = 5 modules
        assert_eq!(netlist.num_modules(), 5);
        // 3 distinct integer nets (string "0" constants excluded)
        assert_eq!(netlist.num_nets(), 3);
    }

    #[test]
    fn test_yosys_no_netnames() {
        let cells = serde_json::json!({
            "buf1": {
                "type": "$buf",
                "connections": {
                    "A": [0],
                    "Y": [1],
                },
            }
        });
        let ports = serde_json::json!({
            "in": {"direction": "input", "bits": [0]},
            "out": {"direction": "output", "bits": [1]},
        });

        let tmp = make_yosys_json(cells, ports, None);
        let netlist = read_yosys_json(tmp.path()).unwrap();

        // 1 cell + 2 ports = 3 modules
        assert_eq!(netlist.num_modules(), 3);
        // 2 distinct nets
        assert_eq!(netlist.num_nets(), 2);
        assert_eq!(netlist.num_pads, 2);
    }

    #[test]
    fn test_yosys_empty_cells() {
        let cells = serde_json::json!({});
        let ports = serde_json::json!({
            "in": {"direction": "input", "bits": [0]},
            "out": {"direction": "output", "bits": [1]},
        });

        let tmp = make_yosys_json(cells, ports, None);
        let netlist = read_yosys_json(tmp.path()).unwrap();

        // 0 cells + 2 ports = 2 modules
        assert_eq!(netlist.num_modules(), 2);
        assert_eq!(netlist.num_nets(), 2);
        assert_eq!(netlist.num_pads, 2);
        assert!(netlist.has_fixed_modules);
    }

    #[test]
    fn test_yosys_invalid_missing_modules() {
        let content = r#"{"not_modules": {}}"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(&tmp, "{}", content).unwrap();
        let result = read_yosys_json(tmp.path());
        assert!(result.is_err());
    }
}
