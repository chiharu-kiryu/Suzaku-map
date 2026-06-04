use suzaku_map::platform::TargetPlatform;
use suzaku_map::platform::android_ime;
use suzaku_map::platform::ime_host_dispatch;
use suzaku_map::platform::panel_companion_dispatch;

fn main() {
    let bootstrap = android_ime::bootstrap_status();
    let ime_dispatch = ime_host_dispatch::dispatch_for(TargetPlatform::Android);
    let panel_dispatch = panel_companion_dispatch::dispatch_for(TargetPlatform::Android);

    println!("Suzaku Android IME Host");
    println!("bootstrap: {}", bootstrap.describe());
    println!("ime-dispatch: {}", ime_dispatch.describe());
    println!("panel-dispatch: {}", panel_dispatch.describe());
}
