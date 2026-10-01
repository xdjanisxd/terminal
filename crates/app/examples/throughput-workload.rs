#[path = "../src/throughput/workloads.rs"]
mod workloads;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let name = args
        .get(1)
        .expect("usage: throughput-workload child|parser WORKLOAD");
    match args[0].as_str() {
        "child" => workloads::child(name),
        "parser" => workloads::parser_baseline(name),
        _ => panic!("expected child or parser"),
    }
}
