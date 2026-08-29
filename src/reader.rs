//! Internal strategy hierarchy for hypergraph file readers.
//!
//! Implements the Strategy and Factory patterns for format-specific parsing —
//! the Rust analog of `source/reader.{hpp,cpp}` in netlistx-cpp. The public
//! API in [`crate::io`] is unchanged: the public functions delegate to these
//! internal readers.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::io::{InputFormat, IoError, IoResult};
use crate::netlist::Netlist;

/// Open an input file for reading.
pub(crate) fn open_input(path: &Path) -> IoResult<BufReader<File>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file))
}

/// Strategy interface: parse one input format into a [`Netlist`].
pub(crate) trait HypergraphReader {
    fn read(&self, path: &Path) -> IoResult<Netlist>;
}

/// hMetis format reader (.hgr, .graph).
pub(crate) struct HmetisReader;

impl HypergraphReader for HmetisReader {
    fn read(&self, path: &Path) -> IoResult<Netlist> {
        let reader = open_input(path)?;
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

            if let Some(net_idx_val) = netlist.get_net_by_name(&net_name) {
                for token in trimmed.split_whitespace() {
                    if let Ok(v) = token.parse::<usize>() {
                        let v_idx = if v > 0 { v - 1 } else { v };
                        if v_idx < num_vertices {
                            let mod_name = format!("m{}", v_idx);
                            if let Some(mod_idx) = netlist.get_module_by_name(&mod_name) {
                                let _ = netlist.add_edge(net_idx_val, mod_idx);
                            }
                        }
                    }
                }
            }

            net_idx += 1;
        }

        Ok(netlist)
    }
}

/// DIMACS hypergraph reader (.dimacs).
pub(crate) struct DimacsReader;

impl HypergraphReader for DimacsReader {
    fn read(&self, path: &Path) -> IoResult<Netlist> {
        let content = std::fs::read_to_string(path)?;

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
}

/// IBM .netD/.net reader.
pub(crate) struct NetDReader;

impl HypergraphReader for NetDReader {
    fn read(&self, path: &Path) -> IoResult<Netlist> {
        let content = std::fs::read_to_string(path)?;
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
            if netlist.get_net_by_name(&net_name).is_none() {
                let _ = netlist.add_net(net_name.clone());
            }

            let mod_name = format!("m{}", node);
            if let (Some(net_idx), Some(mod_idx)) = (
                netlist.get_net_by_name(&net_name),
                netlist.get_module_by_name(&mod_name),
            ) {
                let _ = netlist.add_edge(net_idx, mod_idx);
            }

            pin_count += 1;
        }

        netlist.num_pads = (num_modules - pad_offset - 1) as usize;
        Ok(netlist)
    }
}

/// JSON reader: Yosys JSON if the file has a "modules" key, otherwise node-link JSON.
pub(crate) struct JsonReader;

impl HypergraphReader for JsonReader {
    fn read(&self, path: &Path) -> IoResult<Netlist> {
        let content = std::fs::read_to_string(path)?;
        let data: serde_json::Value = serde_json::from_str(&content)?;

        if data.get("modules").is_some() {
            crate::io::read_yosys_json(path)
        } else {
            crate::io::read_node_link_json(path)
        }
    }
}

/// Factory: create the reader matching `format`.
///
/// `AutoDetect` falls back to the netD reader, matching the historical
/// dispatch in `read_hypergraph()`.
pub(crate) fn make_reader(format: InputFormat) -> Box<dyn HypergraphReader> {
    match format {
        InputFormat::HMetis => Box::new(HmetisReader),
        InputFormat::Json => Box::new(JsonReader),
        InputFormat::Dimacs => Box::new(DimacsReader),
        InputFormat::NetD | InputFormat::AutoDetect => Box::new(NetDReader),
    }
}
