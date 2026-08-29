use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Deserializer as JsonDeserializer;

use crate::netlist::Netlist;
use crate::reader::make_reader;

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
///
/// Dispatches to the internal reader strategy matching `format`.
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

    make_reader(actual_format).read(path.as_ref())
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
            if let Some(idx) = netlist.get_module_by_name(&mod_name) {
                netlist.set_module_weight(idx, *w as i32);
            }
        }
    }

    Ok(())
}

/// Read a netlist from a Yosys JSON file.
///
/// Yosys JSON format contains modules with cells (gates) and ports (I/O).
/// Cells become modules, ports become fixed modules with weight 0.
/// Nets are identified by integer wire IDs from the Yosys representation.
///
/// Ported from Python `read_yosys_json()` in `netlist.py`.
pub fn read_yosys_json<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let reader = crate::reader::open_input(path.as_ref())?;
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

    let mut parts = YosysParts::default();

    // 1. Collect all cells and ports (Map order)
    if let Some(obj) = cells.as_object() {
        parts.cell_names.extend(obj.keys().cloned());
    }
    if let Some(obj) = ports.as_object() {
        parts.port_names.extend(obj.keys().cloned());
    }

    // 2. Collect all unique integer net IDs (skip string constants like "0")
    //    Nets from port bits
    if let Some(ports_obj) = ports.as_object() {
        for port_info in ports_obj.values() {
            if let Some(bits) = port_info.get("bits").and_then(|b| b.as_array()) {
                for bit in bits {
                    if let Some(n) = bit.as_u64() {
                        parts.all_net_ids.insert(n as u32);
                    }
                }
            }
        }
    }

    //    Nets from netnames
    if let Some(netnames) = module_data.get("netnames") {
        if let Some(obj) = netnames.as_object() {
            for netinfo in obj.values() {
                if let Some(bits) = netinfo.get("bits").and_then(|b| b.as_array()) {
                    for bit in bits {
                        if let Some(n) = bit.as_u64() {
                            parts.all_net_ids.insert(n as u32);
                        }
                    }
                }
            }
        }
    }

    //    Nets and edges from cell connections (skip string constants)
    if let Some(cells_obj) = cells.as_object() {
        for (cell_idx, cell_info) in cells_obj.values().enumerate() {
            if let Some(connections) = cell_info.get("connections").and_then(|c| c.as_object()) {
                for conn in connections.values() {
                    if let Some(arr) = conn.as_array() {
                        for net_id in arr {
                            if let Some(n) = net_id.as_u64() {
                                let n = n as u32;
                                parts.all_net_ids.insert(n);
                                parts.cell_edges.push((cell_idx, n));
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Port connections
    if let Some(ports_obj) = ports.as_object() {
        for (port_name, port_info) in ports_obj.iter() {
            let bits: Vec<u32> = port_info
                .get("bits")
                .and_then(|b| b.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|bit| bit.as_u64().map(|n| n as u32))
                        .collect()
                })
                .unwrap_or_default();
            parts.port_bits.insert(port_name.clone(), bits);
        }
    }

    build_netlist_from_parts(parts)
}

// ═══════════════════════════════════════════════════════════════════
//  Streaming JSON parser (SAX) for Yosys netlist files
// ═══════════════════════════════════════════════════════════════════
//
// Each JSON nesting level is handled by a dedicated "seed" type that
// implements DeserializeSeed.  Seeds thread a &mut YosysParts through
// the recursive-descent parse tree — the Rust/serde analogue of nlohmann
// SAX event handlers.

/// Intermediate representation shared by the DOM and SAX Yosys readers.
#[derive(Default)]
struct YosysParts {
    cell_names: Vec<String>,
    all_net_ids: BTreeSet<u32>,
    port_names: Vec<String>,
    port_bits: IndexMap<String, Vec<u32>>,
    cell_edges: Vec<(usize, u32)>,
}

struct TopSeed<'a> {
    data: &'a mut YosysParts,
}
struct ModulesSeed<'a> {
    data: &'a mut YosysParts,
}
struct ModuleSeed<'a> {
    data: &'a mut YosysParts,
}
struct CellsSeed<'a> {
    data: &'a mut YosysParts,
}
struct CellSeed<'a> {
    data: &'a mut YosysParts,
    cell_idx: usize,
}
struct ConnectionsSeed<'a> {
    data: &'a mut YosysParts,
    cell_idx: usize,
}
struct PortsSeed<'a> {
    data: &'a mut YosysParts,
}
struct PortSeed<'a> {
    data: &'a mut YosysParts,
    port_name: String,
}
struct NetnamesSeed<'a> {
    data: &'a mut YosysParts,
}

impl<'de, 'a> DeserializeSeed<'de> for TopSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TopVisitor<'a> {
            data: &'a mut YosysParts,
        }

        impl<'de, 'a> Visitor<'de> for TopVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a Yosys JSON object with 'modules' key")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "modules" => {
                            map.next_value_seed(ModulesSeed { data: self.data })?;
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(TopVisitor { data: self.data })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for ModulesSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ModulesVisitor<'a> {
            data: &'a mut YosysParts,
        }

        impl<'de, 'a> Visitor<'de> for ModulesVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a 'modules' object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                if let Some(_mod_name) = map.next_key::<String>()? {
                    map.next_value_seed(ModuleSeed { data: self.data })?;
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(ModulesVisitor { data: self.data })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for ModuleSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ModuleVisitor<'a> {
            data: &'a mut YosysParts,
        }

        impl<'de, 'a> Visitor<'de> for ModuleVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a module object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "cells" => {
                            map.next_value_seed(CellsSeed { data: self.data })?;
                        }
                        "ports" => {
                            map.next_value_seed(PortsSeed { data: self.data })?;
                        }
                        "netnames" => {
                            map.next_value_seed(NetnamesSeed { data: self.data })?;
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(ModuleVisitor { data: self.data })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for CellsSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CellsVisitor<'a> {
            data: &'a mut YosysParts,
        }

        impl<'de, 'a> Visitor<'de> for CellsVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a 'cells' object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(cell_name) = map.next_key::<String>()? {
                    let cell_idx = self.data.cell_names.len();
                    self.data.cell_names.push(cell_name);
                    map.next_value_seed(CellSeed {
                        data: self.data,
                        cell_idx,
                    })?;
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(CellsVisitor { data: self.data })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for CellSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CellVisitor<'a> {
            data: &'a mut YosysParts,
            cell_idx: usize,
        }

        impl<'de, 'a> Visitor<'de> for CellVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a cell object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "connections" => {
                            map.next_value_seed(ConnectionsSeed {
                                data: self.data,
                                cell_idx: self.cell_idx,
                            })?;
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(CellVisitor {
            data: self.data,
            cell_idx: self.cell_idx,
        })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for ConnectionsSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ConnectionsVisitor<'a> {
            data: &'a mut YosysParts,
            cell_idx: usize,
        }

        impl<'de, 'a> Visitor<'de> for ConnectionsVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a 'connections' object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(_port) = map.next_key::<String>()? {
                    struct ConnArraySeed<'a> {
                        data: &'a mut YosysParts,
                        cell_idx: usize,
                    }

                    impl<'de, 'a> DeserializeSeed<'de> for ConnArraySeed<'a> {
                        type Value = ();

                        fn deserialize<D>(
                            self,
                            deserializer: D,
                        ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
                        where
                            D: Deserializer<'de>,
                        {
                            struct ConnArrayVisitor<'a> {
                                data: &'a mut YosysParts,
                                cell_idx: usize,
                            }

                            impl<'de, 'a> Visitor<'de> for ConnArrayVisitor<'a> {
                                type Value = ();

                                fn expecting(
                                    &self,
                                    f: &mut std::fmt::Formatter,
                                ) -> std::fmt::Result {
                                    f.write_str("array of net IDs")
                                }

                                fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
                                where
                                    A: SeqAccess<'de>,
                                {
                                    while let Some(elem) =
                                        seq.next_element::<serde_json::Value>()?
                                    {
                                        if let Some(n) = elem.as_u64() {
                                            let n = n as u32;
                                            self.data.all_net_ids.insert(n);
                                            self.data.cell_edges.push((self.cell_idx, n));
                                        }
                                    }
                                    Ok(())
                                }
                            }

                            deserializer.deserialize_any(ConnArrayVisitor {
                                data: self.data,
                                cell_idx: self.cell_idx,
                            })
                        }
                    }

                    map.next_value_seed(ConnArraySeed {
                        data: self.data,
                        cell_idx: self.cell_idx,
                    })?;
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(ConnectionsVisitor {
            data: self.data,
            cell_idx: self.cell_idx,
        })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for PortsSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PortsVisitor<'a> {
            data: &'a mut YosysParts,
        }

        impl<'de, 'a> Visitor<'de> for PortsVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a 'ports' object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(port_name) = map.next_key::<String>()? {
                    self.data.port_names.push(port_name.clone());
                    map.next_value_seed(PortSeed {
                        data: self.data,
                        port_name,
                    })?;
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(PortsVisitor { data: self.data })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for PortSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PortVisitor<'a> {
            data: &'a mut YosysParts,
            port_name: String,
        }

        impl<'de, 'a> Visitor<'de> for PortVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a port object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(key) = map.next_key::<String>()? {
                    if key == "bits" {
                        struct PortBitsSeed<'a> {
                            data: &'a mut YosysParts,
                            port_name: String,
                        }

                        impl<'de, 'a> DeserializeSeed<'de> for PortBitsSeed<'a> {
                            type Value = ();

                            fn deserialize<D>(
                                self,
                                deserializer: D,
                            ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
                            where
                                D: Deserializer<'de>,
                            {
                                struct PortBitsVisitor<'a> {
                                    data: &'a mut YosysParts,
                                    port_name: String,
                                }

                                impl<'de, 'a> Visitor<'de> for PortBitsVisitor<'a> {
                                    type Value = ();

                                    fn expecting(
                                        &self,
                                        f: &mut std::fmt::Formatter,
                                    ) -> std::fmt::Result {
                                        f.write_str("array of net IDs")
                                    }

                                    fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
                                    where
                                        A: SeqAccess<'de>,
                                    {
                                        let mut bits = Vec::new();
                                        while let Some(n) = seq.next_element::<u64>()? {
                                            let n = n as u32;
                                            bits.push(n);
                                            self.data.all_net_ids.insert(n);
                                        }
                                        self.data.port_bits.insert(self.port_name.clone(), bits);
                                        Ok(())
                                    }
                                }

                                deserializer.deserialize_any(PortBitsVisitor {
                                    data: self.data,
                                    port_name: self.port_name,
                                })
                            }
                        }

                        map.next_value_seed(PortBitsSeed {
                            data: self.data,
                            port_name: self.port_name.clone(),
                        })?;
                    } else {
                        map.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(PortVisitor {
            data: self.data,
            port_name: self.port_name,
        })
    }
}

impl<'de, 'a> DeserializeSeed<'de> for NetnamesSeed<'a> {
    type Value = ();

    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NetnamesVisitor<'a> {
            data: &'a mut YosysParts,
        }

        impl<'de, 'a> Visitor<'de> for NetnamesVisitor<'a> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a 'netnames' object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
            where
                A: MapAccess<'de>,
            {
                while let Some(_net_name) = map.next_key::<String>()? {
                    struct NetnameEntrySeed<'a> {
                        data: &'a mut YosysParts,
                    }

                    impl<'de, 'a> DeserializeSeed<'de> for NetnameEntrySeed<'a> {
                        type Value = ();

                        fn deserialize<D>(
                            self,
                            deserializer: D,
                        ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
                        where
                            D: Deserializer<'de>,
                        {
                            struct NetnameEntryVisitor<'a> {
                                data: &'a mut YosysParts,
                            }

                            impl<'de, 'a> Visitor<'de> for NetnameEntryVisitor<'a> {
                                type Value = ();

                                fn expecting(
                                    &self,
                                    f: &mut std::fmt::Formatter,
                                ) -> std::fmt::Result {
                                    f.write_str("a netname entry object")
                                }

                                fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
                                where
                                    A: MapAccess<'de>,
                                {
                                    while let Some(key) = map.next_key::<String>()? {
                                        if key == "bits" {
                                            struct BitsSeed<'a> {
                                                data: &'a mut YosysParts,
                                            }

                                            impl<'de, 'a> DeserializeSeed<'de> for BitsSeed<'a> {
                                                type Value = ();

                                                fn deserialize<D>(
                                                    self,
                                                    deserializer: D,
                                                ) -> Result<<Self as DeserializeSeed<'de>>::Value, D::Error>
                                                where
                                                    D: Deserializer<'de>,
                                                {
                                                    struct BitsVisitor<'a> {
                                                        data: &'a mut YosysParts,
                                                    }

                                                    impl<'de, 'a> Visitor<'de> for BitsVisitor<'a> {
                                                        type Value = ();

                                                        fn expecting(
                                                            &self,
                                                            f: &mut std::fmt::Formatter,
                                                        ) -> std::fmt::Result
                                                        {
                                                            f.write_str("array of net IDs")
                                                        }

                                                        fn visit_seq<A>(
                                                            self,
                                                            mut seq: A,
                                                        ) -> Result<(), A::Error>
                                                        where
                                                            A: SeqAccess<'de>,
                                                        {
                                                            while let Some(n) =
                                                                seq.next_element::<u64>()?
                                                            {
                                                                self.data
                                                                    .all_net_ids
                                                                    .insert(n as u32);
                                                            }
                                                            Ok(())
                                                        }
                                                    }

                                                    deserializer.deserialize_any(BitsVisitor {
                                                        data: self.data,
                                                    })
                                                }
                                            }

                                            map.next_value_seed(BitsSeed { data: self.data })?;
                                        } else {
                                            map.next_value::<IgnoredAny>()?;
                                        }
                                    }
                                    Ok(())
                                }
                            }

                            deserializer.deserialize_any(NetnameEntryVisitor { data: self.data })
                        }
                    }

                    map.next_value_seed(NetnameEntrySeed { data: self.data })?;
                }
                Ok(())
            }
        }

        deserializer.deserialize_any(NetnamesVisitor { data: self.data })
    }
}

/// Read a netlist from a Yosys JSON file using SAX-style streaming parsing.
///
/// Processes the JSON as a stream of events without building the full DOM tree.
/// Only the first module is processed (matching `read_yosys_json` behavior).
/// More memory-efficient than the DOM version for large files.
///
/// # Arguments
///
/// * `path` - Path to the Yosys JSON file
///
/// # Errors
///
/// Returns an error if the file cannot be read, the JSON is malformed, or
/// required keys ("modules", "cells", "ports") are missing.
pub fn read_yosys_json_sax<P: AsRef<Path>>(path: P) -> IoResult<Netlist> {
    let reader = crate::reader::open_input(path.as_ref())?;
    let mut de = JsonDeserializer::from_reader(reader);

    let mut parts = YosysParts::default();

    TopSeed { data: &mut parts }
        .deserialize(&mut de)
        .map_err(IoError::JsonError)?;

    if parts.cell_names.is_empty() && parts.port_names.is_empty() {
        return Err(IoError::InvalidFormat("Missing 'modules' key".to_string()));
    }

    build_netlist_from_parts(parts)
}

/// Build a Netlist from parsed Yosys parts (shared phase 2 of the DOM and
/// SAX readers).
fn build_netlist_from_parts(data: YosysParts) -> IoResult<Netlist> {
    let cell_names = data.cell_names;
    let port_names = data.port_names;
    let nets_list: Vec<u32> = data.all_net_ids.into_iter().collect();
    let num_ports = port_names.len();

    let mut netlist = Netlist::new();

    for cell_name in &cell_names {
        netlist
            .add_module(cell_name.clone())
            .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
    }

    for port_name in &port_names {
        netlist
            .add_module(format!("PORT_{}", port_name))
            .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
    }

    for &net_id in &nets_list {
        netlist
            .add_net(net_id.to_string())
            .map_err(|e| IoError::InvalidFormat(e.to_string()))?;
    }

    for &(cell_idx, net_id) in &data.cell_edges {
        if nets_list.contains(&net_id) {
            let net_name = net_id.to_string();
            let cell_name = &cell_names[cell_idx];
            if let (Some(net_idx), Some(cell_idx_val)) = (
                netlist.get_net_by_name(&net_name),
                netlist.get_module_by_name(cell_name),
            ) {
                let _ = netlist.add_edge(net_idx, cell_idx_val);
            }
        }
    }

    for port_name in &port_names {
        if let Some(bits) = data.port_bits.get(port_name) {
            for &net_id in bits {
                if nets_list.contains(&net_id) {
                    let net_name = net_id.to_string();
                    let port_mod = format!("PORT_{}", port_name);
                    if let (Some(net_idx), Some(mod_idx)) = (
                        netlist.get_net_by_name(&net_name),
                        netlist.get_module_by_name(&port_mod),
                    ) {
                        let _ = netlist.add_edge(net_idx, mod_idx);
                    }
                }
            }
        }
    }

    netlist.num_pads = num_ports;

    for cell_name in &cell_names {
        if let Some(idx) = netlist.get_module_by_name(cell_name) {
            netlist.set_module_weight(idx, 1);
        }
    }
    for port_name in &port_names {
        let port_mod = format!("PORT_{}", port_name);
        if let Some(idx) = netlist.get_module_by_name(&port_mod) {
            netlist.set_module_weight(idx, 0);
        }
    }

    for port_name in &port_names {
        let port_mod = format!("PORT_{}", port_name);
        if let Some(idx) = netlist.get_module_by_name(&port_mod) {
            netlist.module_fixed.insert(idx);
        }
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
        .ok_or_else(|| IoError::InvalidFormat("Missing num_modules".to_string()))?
        as usize;
    let _num_nets = graph_obj
        .get("num_nets")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| IoError::InvalidFormat("Missing num_nets".to_string()))?
        as usize;
    let num_pads = graph_obj
        .get("num_pads")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let nodes = data
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| IoError::InvalidFormat("Missing 'nodes' array".to_string()))?;

    // First num_modules entries are modules, rest are nets
    let mut netlist = Netlist::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = node
            .get("id")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
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
                .and_then(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                })
                .ok_or_else(|| IoError::InvalidFormat("Edge missing valid 'source'".to_string()))?;
            let target = edge
                .get("target")
                .and_then(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                })
                .ok_or_else(|| IoError::InvalidFormat("Edge missing valid 'target'".to_string()))?;
            // In node-link format, we don't know which direction the edge goes.
            // Try both (net, module) and (module, net) orders.
            if let Some(net_idx) = netlist.get_net_by_name(&source) {
                if let Some(mod_idx) = netlist.get_module_by_name(&target) {
                    let _ = netlist.add_edge(net_idx, mod_idx);
                }
            } else if let Some(net_idx) = netlist.get_net_by_name(&target) {
                if let Some(mod_idx) = netlist.get_module_by_name(&source) {
                    let _ = netlist.add_edge(net_idx, mod_idx);
                }
            }
        }
    }

    netlist.num_pads = num_pads;

    // Set module weights from node attributes
    for (i, node) in nodes.iter().enumerate() {
        if i < num_modules {
            if let Some(id) = node.get("id").and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            }) {
                if let Some(w) = node.get("weight").and_then(|v| v.as_i64()) {
                    if let Some(idx) = netlist.get_module_by_name(&id) {
                        netlist.set_module_weight(idx, w as i32);
                    }
                }
            }
        }
    }

    Ok(netlist)
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
    for module_idx in netlist.module_indices() {
        writeln!(
            file,
            "  {{ \"id\": \"{}\" }},",
            &netlist.module_names[module_idx]
        )?;
    }
    for net_idx in netlist.net_indices() {
        writeln!(file, "  {{ \"id\": \"{}\" }},", &netlist.net_names[net_idx])?;
    }
    writeln!(file, " ],")?;

    writeln!(file, " \"links\": [")?;
    for module_idx in netlist.module_indices() {
        let module_name = &netlist.module_names[module_idx];
        for net_idx in netlist.get_module_nets(module_idx) {
            let net_name = &netlist.net_names[net_idx];
            writeln!(file, "  {{")?;
            writeln!(file, "   \"source\": \"{}\",", module_name)?;
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
    for module_idx in netlist.module_indices() {
        writeln!(file, "MODULE {}", &netlist.module_names[module_idx])?;
    }
    writeln!(file)?;

    writeln!(file, "# Nets")?;
    for net_idx in netlist.net_indices() {
        let modules = netlist.get_net_modules(net_idx);
        if !modules.is_empty() {
            let mod_names: Vec<String> = modules
                .iter()
                .map(|&i| netlist.module_names[i].clone())
                .collect();
            writeln!(
                file,
                "NET {} {}",
                &netlist.net_names[net_idx],
                mod_names.join(" ")
            )?;
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
        let n1 = netlist.get_net_by_name("n1").unwrap();
        let n2 = netlist.get_net_by_name("n2").unwrap();
        let m1 = netlist.get_module_by_name("m1").unwrap();
        let m2 = netlist.get_module_by_name("m2").unwrap();
        let m3 = netlist.get_module_by_name("m3").unwrap();
        netlist.add_edge(n1, m1).unwrap();
        netlist.add_edge(n1, m2).unwrap();
        netlist.add_edge(n2, m2).unwrap();
        netlist.add_edge(n2, m3).unwrap();

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
        let n1 = netlist.get_net_by_name("n1").unwrap();
        let n2 = netlist.get_net_by_name("n2").unwrap();
        let m1 = netlist.get_module_by_name("m1").unwrap();
        let m2 = netlist.get_module_by_name("m2").unwrap();
        let m3 = netlist.get_module_by_name("m3").unwrap();
        netlist.add_edge(n1, m1).unwrap();
        netlist.add_edge(n1, m2).unwrap();
        netlist.add_edge(n2, m2).unwrap();
        netlist.add_edge(n2, m3).unwrap();

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
        assert_eq!(netlist.gr.edge_count(), 6);
        assert_eq!(netlist.num_pads, 3);

        // Cells have weight 1
        let and1_idx = netlist.get_module_by_name("and1").unwrap();
        assert_eq!(netlist.get_module_weight(and1_idx), 1);
        // Ports have weight 0
        let port_a_idx = netlist.get_module_by_name("PORT_a").unwrap();
        let port_b_idx = netlist.get_module_by_name("PORT_b").unwrap();
        let port_y_idx = netlist.get_module_by_name("PORT_y").unwrap();
        assert_eq!(netlist.get_module_weight(port_a_idx), 0);
        assert_eq!(netlist.get_module_weight(port_b_idx), 0);
        assert_eq!(netlist.get_module_weight(port_y_idx), 0);

        // Ports are fixed
        assert!(netlist.module_fixed.contains(&port_a_idx));
        assert!(netlist.module_fixed.contains(&port_b_idx));
        assert!(netlist.module_fixed.contains(&port_y_idx));
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

    // --- SAX version tests (should match the DOM version) ---

    #[test]
    fn test_yosys_sax_simple_and_gate() {
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
        let netlist = read_yosys_json_sax(tmp.path()).unwrap();

        assert_eq!(netlist.num_modules(), 4);
        assert_eq!(netlist.num_nets(), 3);
        assert_eq!(netlist.number_of_nodes(), 7);
        assert_eq!(netlist.gr.edge_count(), 6);
        assert_eq!(netlist.num_pads, 3);

        let and1_idx = netlist.get_module_by_name("and1").unwrap();
        assert_eq!(netlist.get_module_weight(and1_idx), 1);
        let port_a_idx = netlist.get_module_by_name("PORT_a").unwrap();
        assert_eq!(netlist.get_module_weight(port_a_idx), 0);
        assert!(netlist.module_fixed.contains(&port_a_idx));
        assert!(netlist.has_fixed_modules);
    }

    #[test]
    fn test_yosys_sax_two_cells_shared_net() {
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
        let netlist = read_yosys_json_sax(tmp.path()).unwrap();

        assert_eq!(netlist.num_modules(), 4);
        assert_eq!(netlist.num_nets(), 3);
        assert_eq!(netlist.num_pads, 2);
        assert_eq!(netlist.number_of_nodes(), 7);
    }

    #[test]
    fn test_yosys_sax_ignores_string_constants() {
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
        let netlist = read_yosys_json_sax(tmp.path()).unwrap();

        assert_eq!(netlist.num_modules(), 5);
        assert_eq!(netlist.num_nets(), 3);
    }

    #[test]
    fn test_yosys_sax_no_netnames() {
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
        let netlist = read_yosys_json_sax(tmp.path()).unwrap();

        assert_eq!(netlist.num_modules(), 3);
        assert_eq!(netlist.num_nets(), 2);
        assert_eq!(netlist.num_pads, 2);
    }

    #[test]
    fn test_yosys_sax_empty_cells() {
        let cells = serde_json::json!({});
        let ports = serde_json::json!({
            "in": {"direction": "input", "bits": [0]},
            "out": {"direction": "output", "bits": [1]},
        });

        let tmp = make_yosys_json(cells, ports, None);
        let netlist = read_yosys_json_sax(tmp.path()).unwrap();

        assert_eq!(netlist.num_modules(), 2);
        assert_eq!(netlist.num_nets(), 2);
        assert_eq!(netlist.num_pads, 2);
        assert!(netlist.has_fixed_modules);
    }

    #[test]
    fn test_yosys_sax_matches_dom_on_real_file() {
        // Verify SAX produces the same result as DOM for a real-ish Yosys JSON
        let cells = serde_json::json!({
            "and1": {
                "type": "$and",
                "connections": {"A": [0], "B": [1], "Y": [2]},
            }
        });
        let ports = serde_json::json!({
            "a": {"direction": "input", "bits": [0]},
            "b": {"direction": "input", "bits": [1]},
            "y": {"direction": "output", "bits": [2]},
        });

        let tmp = make_yosys_json(cells, ports, None);
        let dom_netlist = read_yosys_json(tmp.path()).unwrap();
        let sax_netlist = read_yosys_json_sax(tmp.path()).unwrap();

        assert_eq!(dom_netlist.num_modules(), sax_netlist.num_modules());
        assert_eq!(dom_netlist.num_nets(), sax_netlist.num_nets());
        assert_eq!(dom_netlist.num_pads, sax_netlist.num_pads);
        assert_eq!(dom_netlist.number_of_nodes(), sax_netlist.number_of_nodes());
        assert_eq!(dom_netlist.gr.edge_count(), sax_netlist.gr.edge_count());
        assert_eq!(dom_netlist.module_fixed, sax_netlist.module_fixed);
    }

    #[test]
    fn test_yosys_sax_invalid_missing_modules() {
        let content = r#"{"not_modules": {}}"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        write!(&tmp, "{}", content).unwrap();
        let result = read_yosys_json_sax(tmp.path());
        assert!(result.is_err());
    }
}
