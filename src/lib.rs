use std::sync::mpsc::Sender;
use std::sync::Mutex;

use windows::core::{IInspectable, Result, HSTRING};
use windows::Devices::WiFiDirect::{
    WiFiDirectAdvertisementPublisher, WiFiDirectAdvertisementPublisherStatus,
    WiFiDirectAdvertisementPublisherStatusChangedEventArgs, WiFiDirectConnectionListener,
    WiFiDirectConnectionRequestedEventArgs, WiFiDirectConnectionStatus, WiFiDirectDevice,
    WiFiDirectError,
};
use windows::Foundation::{AsyncOperationCompletedHandler, AsyncStatus, TypedEventHandler};
use windows::Security::Credentials::PasswordCredential;

pub struct WlanHostedNetworkHelper {
    publisher: Mutex<WiFiDirectAdvertisementPublisher>,
    tx: Mutex<Sender<String>>, // mutex necessary for integration with tokio
}

impl WlanHostedNetworkHelper {
    pub fn new(ssid: &str, password: &str, tx: Sender<String>) -> Result<Self> {
        let publisher = start(ssid, password, tx.clone())?;
        Ok(WlanHostedNetworkHelper {
            publisher: Mutex::new(publisher),
            tx: Mutex::new(tx),
        })
    }

    pub fn stop(&self) -> Result<()> {
        let publisher = self
            .publisher
            .lock()
            .expect("Couldn't lock publisher mutex.");
        let status = publisher.Status()?;
        if status == WiFiDirectAdvertisementPublisherStatus::Started {
            publisher.Stop()?;
            // self.tx
            //     .lock()
            //     .expect("Couldn't lock sender mutex.")
            //     .send("Hosted network stopped".to_string())
            //     .expect("Could not send on channel.");
        } else {
            self.tx
                .lock()
                .expect("Couldn't lock sender mutex.")
                .send("Stop called but WiFiDirectAdvertisementPublisher is not running".to_string())
                .expect("Could not send on channel.");
        }
        Ok(())
    }
}

fn start_listener(tx: Sender<String>) -> Result<()> {
    let listener = WiFiDirectConnectionListener::new()?;
    let connection_requested_callback = TypedEventHandler::<
        WiFiDirectConnectionListener,
        WiFiDirectConnectionRequestedEventArgs,
    >::new(move |_sender, args| {
        tx.send("Connection requested...".to_string())
            .expect("Couldn't send on tx");
        let request = args
            .as_ref()
            .expect("args == None in connection requested callback")
            .GetConnectionRequest()?;
        let device_info = request.DeviceInformation()?;
        let device_id = device_info.Id()?;
        let wifi_direct_device = WiFiDirectDevice::FromIdAsync(&device_id)?;
        let async_operation_completed_callback =
            AsyncOperationCompletedHandler::<WiFiDirectDevice>::new(|async_operation, status| {
                if status == AsyncStatus::Completed {
                    let wfd_device = async_operation
                        .as_ref()
                        .expect("No device in WiFiDirectDevice AsyncOperation callback")
                        .GetResults()?;
                    let endpoint_pairs = wfd_device.GetConnectionEndpointPairs()?;
                    let endpoint_pair = endpoint_pairs.GetAt(0)?;
                    let remote_hostname = endpoint_pair.RemoteHostName()?;
                    let _display_name = remote_hostname.DisplayName();
                    let connection_status_changed_callback = TypedEventHandler::<
                        WiFiDirectDevice,
                        IInspectable,
                    >::new(
                        |sender, _inspectable| {
                            let status = sender
                                .as_ref()
                                .expect("No sender in connection status changed handler")
                                .ConnectionStatus()?;
                            // TODO: do we need to do anything here? We don't need to keep track of multiple clients.
                            // C++ seems to store them in a map but not use them? It does call remove_ConnectionStatusChanged() on the tokens when this disconnected branch hits...
                            // So I'd like to replicate, but don't know how to reference a map of device IDs and tokens. Arc?
                            match status {
                                WiFiDirectConnectionStatus::Disconnected => {
                                    let _device_id = sender
                                        .as_ref()
                                        .expect("No sender in connection status changed handler")
                                        .DeviceId()?;
                                }
                                _ => (),
                            }
                            Ok(())
                        },
                    );
                    // In https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/WiFiDirectLegacyAP/cpp/WlanHostedNetworkWinRT.cpp,
                    // they store this token and the device ID in maps to keep track of connected clients. they don't seem to do anything with them though.
                    // skipping now as it's not necessary for our purposes.
                    let _event_registration_token =
                        wfd_device.ConnectionStatusChanged(&connection_status_changed_callback);
                }
                Ok(())
            });
        wifi_direct_device.SetCompleted(&async_operation_completed_callback)?;
        Ok(())
    });
    listener.ConnectionRequested(&connection_requested_callback)?;
    Ok(())
}

fn start(
    ssid: &str,
    password: &str,
    tx: Sender<String>,
) -> Result<WiFiDirectAdvertisementPublisher> {
    let publisher = WiFiDirectAdvertisementPublisher::new()?;

    // add status changed handler
    let _ssid = ssid.to_string();
    let publisher_status_changed_callback = TypedEventHandler::<
        WiFiDirectAdvertisementPublisher,
        WiFiDirectAdvertisementPublisherStatusChangedEventArgs,
    >::new(move |_sender, args| {
        let status = args
            .as_ref()
            .expect("args == None in status change callback")
            .Status()?;
        match status {
            WiFiDirectAdvertisementPublisherStatus::Created => tx
                .send("Hosted network created".to_string())
                .expect("Couldn't send on tx"),
            WiFiDirectAdvertisementPublisherStatus::Stopped => tx
                .send("Hosted network stopped".to_string())
                .expect("Couldn't send on tx"),
            WiFiDirectAdvertisementPublisherStatus::Started => {
                start_listener(tx.clone())?;
                tx.send(format!("Hosted network {} has started", _ssid))
                    .expect("Couldn't send on tx");
            }
            WiFiDirectAdvertisementPublisherStatus::Aborted => {
                let err = match args
                    .as_ref()
                    .expect("args == None in status change callback")
                    .Error()
                    .expect("Couldn't get error")
                {
                    WiFiDirectError::RadioNotAvailable => "Radio not available",
                    WiFiDirectError::ResourceInUse => "Resource in use",
                    WiFiDirectError::Success => "Success",
                    _ => panic!("got bad WiFiDirectError"),
                };
                tx.send(format!("Hosted network aborted: {}", err))
                    .expect("Couldn't send on tx");
            }
            _ => panic!("Bad status received in callback."),
        }
        Ok(())
    });
    publisher.StatusChanged(&publisher_status_changed_callback)?;

    // set advertisement required settings
    let advertisement = publisher
        .Advertisement()
        .expect("Error getting advertisement");
    advertisement.SetIsAutonomousGroupOwnerEnabled(true)?;

    // set ssid and password
    let legacy_settings = advertisement.LegacySettings()?;
    legacy_settings.SetIsEnabled(true)?;
    let _ssid = HSTRING::from(ssid);
    legacy_settings.SetSsid(&_ssid)?;
    let password_credential = PasswordCredential::new()?;
    password_credential.SetPassword(&HSTRING::from(password))?;
    legacy_settings.SetPassphrase(&password_credential)?;

    // Start the advertisement, which will create an access point that other peers can connect to
    publisher.Start()?;

    Ok(publisher)
}

#[cfg(test)]
mod tests {
    use crate::WlanHostedNetworkHelper;
    use std::sync::mpsc;
    use std::thread::spawn;

    // run with `cargo test -- --nocapture` to see output
    #[test]
    fn run_hosted_network() {
        // Make channels to receive messages from Windows Runtime
        let (tx, rx) = mpsc::channel::<String>();
        let wlan_hosted_network_helper =
            WlanHostedNetworkHelper::new("WiFiDirectTestNetwork", "TestingThisLibrary", tx)
                .unwrap();

        spawn(move || loop {
            let msg = match rx.recv() {
                Ok(m) => m,
                Err(e) => {
                    println!("WiFiDirect thread exiting: {}", e);
                    break;
                }
            };
            println!("{}", msg);
        });

        // Use the hosted network
        std::thread::sleep(std::time::Duration::from_secs(10));

        // Stop it when done
        wlan_hosted_network_helper.stop().expect("Error in stop()");
    }
}
