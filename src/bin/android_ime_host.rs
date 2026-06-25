use suzaku_map::platform::TargetPlatform;
use suzaku_map::platform::ime_host_runtime::runtime_report_for;

fn main() {
    println!("{}", runtime_report_for(TargetPlatform::Android, true));
}
