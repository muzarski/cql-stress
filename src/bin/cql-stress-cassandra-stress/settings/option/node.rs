use std::{
    fs::File,
    io::{self, BufRead},
};

use anyhow::{Context, Result};

use crate::settings::{
    param::{ParamsParser, SimpleParamHandle},
    ParsePayload,
};

pub struct NodeOption {
    pub nodes: Vec<String>,
    pub whitelist: bool,
    pub datacenter: Option<String>,
}

impl NodeOption {
    pub const CLI_STRING: &str = "-node";

    pub fn description() -> &'static str {
        "Nodes to connect to"
    }

    pub fn parse(cl_args: &mut ParsePayload) -> Result<Self> {
        let params = cl_args.remove(Self::CLI_STRING).unwrap_or_default();
        let (parser, handles) = prepare_parser();
        parser.parse(params)?;
        Self::from_handles(handles)
    }

    pub fn print_help() {
        let (parser, _) = prepare_parser();
        parser.print_help();
    }

    pub fn print_settings(&self) {
        println!("Node:");
        println!("  Nodes: {:?}", self.nodes);
        println!("  Is White List: {}", self.whitelist);
        println!("  Datacenter: {:?}", self.datacenter);
    }

    fn from_handles(handles: NodeParamHandles) -> Result<NodeOption> {
        let datacenter = handles.datacenter.get();
        let whitelist = handles.whitelist.supplied_by_user();
        let file = handles.file.get();
        let nodes = handles.nodes.get();

        let nodes = match nodes {
            Some(nodes) => nodes.split(',').map(|nd| nd.to_owned()).collect(),
            // SAFETY: Parameters are grouped in a way that either `nodes` or `file` is Some.
            // Note that it's never the case that both of them are Some.
            _ => read_nodes_from_file(&file.unwrap())?,
        };

        Ok(Self {
            nodes,
            whitelist,
            datacenter,
        })
    }
}

struct NodeParamHandles {
    datacenter: SimpleParamHandle,
    whitelist: SimpleParamHandle,
    file: SimpleParamHandle,
    nodes: SimpleParamHandle,
}

fn prepare_parser() -> (ParamsParser, NodeParamHandles) {
    let mut parser = ParamsParser::new(NodeOption::CLI_STRING);

    let datacenter = parser.simple_param(
        "datacenter=",
        r"^.*$",
        None,
        "Preferred datacenter for the default load balancing policy",
        false,
    );
    let whitelist = parser.simple_param(
        "whitelist",
        r"^$",
        None,
        "Limit communications to the provided nodes",
        false,
    );
    let file = parser.simple_param("file=", r"^.*$", None, "Node file (one per line)", false);
    let nodes = parser.simple_param(
        "",
        r"^[^=,]+(,[^=,]+)*$",
        Some("localhost"),
        "comma delimited list of nodes",
        false,
    );

    // $ ./cassandra-stress help -node
    // Usage: -node [datacenter=?] [whitelist] []
    //  OR
    // Usage: -node [datacenter=?] [whitelist] [file=?]
    parser.group(&[&datacenter, &whitelist, &nodes]);
    parser.group(&[&datacenter, &whitelist, &file]);

    (
        parser,
        NodeParamHandles {
            datacenter,
            whitelist,
            file,
            nodes,
        },
    )
}

fn read_nodes_from_file(filename: &str) -> Result<Vec<String>> {
    let file = File::open(filename).context("Invalid nodes file")?;
    let buf = io::BufReader::new(file);
    buf.lines()
        // Filter out empty lines.
        .filter(|s| !s.as_ref().is_ok_and(String::is_empty))
        .collect::<Result<Vec<_>, _>>()
        .context("Invalid nodes file")
}

#[cfg(test)]
mod tests {
    use node::NodeOption;

    use crate::settings::option::node;

    use super::prepare_parser;

    #[test]
    fn node_good_params_test() {
        let args = vec!["whitelist", "127.0.0.1,localhost,192.168.0.1"];
        let (parser, handles) = prepare_parser();

        assert!(parser.parse(args).is_ok());

        let params = NodeOption::from_handles(handles).unwrap();
        assert_eq!(None, params.datacenter);
        assert!(params.whitelist);
        assert_eq!(vec!["127.0.0.1", "localhost", "192.168.0.1"], params.nodes);
    }

    #[test]
    fn node_bad_params_test() {
        let args = vec!["whitelist", "127.0.0.1,localhost,192.168.0.1,"];
        let (parser, _) = prepare_parser();

        assert!(parser.parse(args).is_err());
    }
}
