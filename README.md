# WiFiDirect Legacy AP (for Windows)

This is a loose Rust adaptation of [Microsoft's C++ WiFi Direct Legacy AP sample code](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/WiFiDirectLegacyAP), adapted for my purposes with [Flying Carpet](https://flyingcarpet.spiegl.dev), and written with [Microsoft's Rust bindings for the Windows API](https://github.com/microsoft/windows-rs). It is a library exposing one struct, `WlanHostedNetworkHelper`, and one trait, `UI`.

## Use

Provide `WlanHostedNetworkHelper::new()` with any type that implements UI:

```
pub trait UI: Clone + Send {
    fn output(&self, msg: &str);
}
```

The `output()` method will be called whenever the Windows Runtime sends any messages about the hosted network.

## Example

```
use wifidirect_legacy_ap::{UI, WlanHostedNetworkHelper};

// Meant to stand in for a Tauri window but can be anything. Must implement Send + Clone.
#[derive(Clone)]
struct Window {
    value: u8, // Value is arbitrary, just demonstrating that we can access struct fields in the output() method
}

impl UI for Window {
    fn output(&self, msg: &str) {
        println!("val: {}, msg: {}", self.value, msg);
    }
}

fn main() {
    // Make a struct that implements UI. Use it and our SSID/password to create and start a hosted network (soft AP, hotspot, whatever you want to call it).
    let window = Window { value: 32 };
    let wlan_hosted_network_helper =
        WlanHostedNetworkHelper::new("WiFiDirectTestNetwork", "TestingThisLibrary", window).unwrap();

    // Use the hosted network
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Stop it when done
    wlan_hosted_network_helper.stop().expect("Error in stop()");
}
```
