# WiFiDirect Legacy AP (for Windows)

This is a loose Rust adaptation of [Microsoft's C++ WiFi Direct Legacy AP sample code](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/WiFiDirectLegacyAP), adapted for my purposes with [Flying Carpet](https://flyingcarpet.spiegl.dev), and written with [Microsoft's Rust bindings for the Windows API](https://github.com/microsoft/windows-rs). It is a library exposing one struct, `WlanHostedNetworkHelper`.


## Example Use

Provide `WlanHostedNetworkHelper::new()` with an SSID, a password, a message `Sender` channel that will be used to write messages back to your code from the Windows Runtime, and a success `Sender` channel that will send one `bool` when the AP starts or fails to start. Keep the returned hotspot in scope for as long as you need it.

```
use std::sync::mpsc;
use std::thread::spawn;
use crate::WlanHostedNetworkHelper;

fn run_hosted_network() {
    // Make channels to receive messages from Windows Runtime
    let (message_tx, message_rx) = mpsc::channel::<String>();
    let (success_tx, success_rx) = mpsc::channel::<bool>();
    let wlan_hosted_network_helper =
        WlanHostedNetworkHelper::new("WiFiDirectTestNetwork", "TestingThisLibrary", message_tx, success_tx)
            .unwrap();

    // Listen for messages in a thread that will exit when the hotspot is done and the sender closes
    spawn(move || loop {
        let msg = match message_rx.recv() {
            Ok(m) => m,
            Err(e) => {
                println!("WiFiDirect thread exiting: {}", e);
                break;
            }
        };
        println!("{}", msg);
    });

    // Wait to see whether we were able to start the hotspot
    let started = success_rx.recv().unwrap();
    if !started {
        panic!("Failed to start hotspot");
    }

    // Use the hosted network
    std::thread::sleep(std::time::Duration::from_secs(10));

    // Stop it when done
    wlan_hosted_network_helper.stop().expect("Error in stop()");
}
```
