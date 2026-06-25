use suzaku_map::platform::ime_host_runtime::current_runtime_report;

fn main() {
    println!("{}", current_runtime_report(true));
}
