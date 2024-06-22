// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use btleplug::api::{BDAddr, Peripheral, Central, CentralEvent, Manager as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use flexi_logger::{FileSpec, Logger, DeferredNow, Duplicate, Record};
use futures::stream::StreamExt;
use std::error::Error;

const HEALTH_SCALE_ADDR: [u8; 6] = const_decoder::Decoder::Hex.decode(b"3403DE08C7B9");

async fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().await.unwrap();
    adapters.into_iter().nth(0).unwrap()
}

pub fn my_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    // let level = record.level();
    write!(
        w,
        "{}: {}",
        now.format("%Y-%m-%d %H:%M:%S"),
        &record.args()
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let manager = Manager::new().await?;

    // get the first bluetooth adapter
    // connect to the adapter
    let central = get_central(&manager).await;

    // Each adapter has an event stream, we fetch via events(),
    // simplifying the type, this will return what is essentially a
    // Future<Result<Stream<Item=CentralEvent>>>.
    let mut events = central.events().await?;

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await?;

    Logger::try_with_env_or_str("info")?
        .log_to_file(
            FileSpec::default()
                .basename("health-scale")
                .suppress_timestamp()
        )
        .duplicate_to_stdout(Duplicate::Debug)
        .append()
        .format_for_files(my_format)
        .print_message()                         //
        .start()?;

    let mut scale = None;

    // Print based on whatever the event receiver outputs. Note that the event
    // receiver blocks, so in a real program, this should be run in its own
    // thread (not task, as this library does not yet use async channels).
    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                if scale.is_none() {
                    if let Ok(peri) = central.peripheral(&id).await {
                        if peri.address() == BDAddr::from(HEALTH_SCALE_ADDR) {
                            scale = Some(id);
                            log::info!("scale,{:?}", peri.address());
                        }
                    }
                }
            }
            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } if Some(&id) == scale.as_ref() => {
                if let Some(data) = manufacturer_data.get(&0) {
                    if let Some(data) = data.get(9..11) {
                        log::debug!("bytes,{data:?}");

                        let val = u16::from_le_bytes([data[0], data[1]]);

                        if val > 0 {
                            log::info!("weight,{:.1}", val as f32 / 100.0 * 2.2046);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
