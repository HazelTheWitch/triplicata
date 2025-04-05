use std::process::ExitCode;

use anyhow::bail;
use btleplug::{
    api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter},
    platform::{Adapter, Manager, PeripheralId},
};
use futures::StreamExt;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

async fn scan_for_cubes(adapter: &Adapter) -> anyhow::Result<PeripheralId> {
    let mut events = adapter.events().await?;
    adapter.start_scan(ScanFilter::default()).await?;

    info!("Scanning for devices...");

    while let Some(event) = events.next().await {
        if let CentralEvent::DeviceDiscovered(id) = event {
            let peripheral = adapter.peripheral(&id).await?;
            let properties = peripheral.properties().await?;
            let Some(name) = properties.and_then(|p| p.local_name) else {
                continue;
            };

            if name.starts_with("GAN") {
                return Ok(id);
            }
        }
    }

    bail!("Could not connect to GAN cube");
}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let manager = Manager::new().await?;

    let mut adapter_list = manager.adapters().await?;

    if adapter_list.is_empty() {
        error!("Could not find bluetooth adapter");
        return Ok(ExitCode::FAILURE);
    }

    let adapter = adapter_list.swap_remove(0);
    let adapter_state = adapter.adapter_state().await?;

    info!("Using adapter: {}", adapter.adapter_info().await?);

    let cube_id = scan_for_cubes(&adapter).await?;
    let cube = adapter.peripheral(&cube_id).await?;

    info!(
        "Found cube: {}",
        cube.properties().await?.unwrap().local_name.unwrap()
    );

    cube.connect().await?;
    cube.discover_services().await?;

    let characteristics = cube.characteristics();

    let mut v1_version = None;
    let mut v1_hardware = None;
    let mut v1_cube_state = None;
    let mut v1_last_moves = None;
    let mut v1_timing = None;
    let mut v1_battery = None;
    let mut v2_write = None;
    let mut v2_read = None;

    for characteristic in characteristics {
        match characteristic.uuid.to_string().as_str() {
            "00002a28-0000-1000-8000-00805f9b34fb" => v1_version = Some(characteristic),
            "00002a23-0000-1000-8000-00805f9b34fb" => v1_hardware = Some(characteristic),
            "0000fff2-0000-1000-8000-00805f9b34fb" => v1_cube_state = Some(characteristic),
            "0000fff5-0000-1000-8000-00805f9b34fb" => v1_last_moves = Some(characteristic),
            "0000fff6-0000-1000-8000-00805f9b34fb" => v1_timing = Some(characteristic),
            "0000fff7-0000-1000-8000-00805f9b34fb" => v1_battery = Some(characteristic),
            "28be4a4a-cd67-11e9-a32f-2a2ae2dbcce4" => v2_write = Some(characteristic),
            "28be4cb6-cd67-11e9-a32f-2a2ae2dbcce4" => v2_read = Some(characteristic),
            id => warn!("Unknown characteristic: {id}"),
        }
    }

    println!("{v1_version:?}");
    println!("{v1_hardware:?}");
    println!("{v1_cube_state:?}");
    println!("{v1_last_moves:?}");
    println!("{v1_timing:?}");
    println!("{v1_battery:?}");
    println!("{v2_write:?}");
    println!("{v2_read:?}");

    Ok(ExitCode::SUCCESS)
}
