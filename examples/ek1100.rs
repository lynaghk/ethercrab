//! Demonstrate setting outputs using a Beckhoff EK1100 and modules.
//!
//! Run with e.g.
//!
//! Linux
//!
//! ```bash
//! RUST_LOG=debug cargo run --example ek1100 --release -- eth0
//! ```
//!
//! Windows
//!
//! ```ps
//! $env:RUST_LOG="debug" ; cargo run --example ek1100 --release -- '\Device\NPF_{FF0ACEE6-E8CD-48D5-A399-619CD2340465}'
//! ```

use async_ctrlc::CtrlC;
use async_io::Timer;
use ethercrab::{
    error::Error, std::tx_rx_task, Client, ClientConfig, PduStorage, SlaveGroup, SubIndex, Timeouts,
};
use futures_lite::{FutureExt, StreamExt};
use smol::LocalExecutor;
use std::{sync::Arc, time::Duration};

/// Maximum number of slaves that can be stored.
const MAX_SLAVES: usize = 16;
/// Maximum PDU data payload size - set this to the max PDI size or higher.
const MAX_PDU_DATA: usize = 1100;
/// Maximum number of EtherCAT frames that can be in flight at any one time.
const MAX_FRAMES: usize = 16;
/// Maximum total PDI length.
const PDI_LEN: usize = 64;

static PDU_STORAGE: PduStorage<MAX_FRAMES, MAX_PDU_DATA> = PduStorage::new();

async fn main_inner(ex: &LocalExecutor<'static>) -> Result<(), Error> {
    let interface = std::env::args()
        .nth(1)
        .expect("Provide interface as first argument");

    log::info!("Starting EK1100 demo...");
    log::info!("Ensure an EK1100 is the first slave, with any number of modules connected after");

    let (tx, rx, pdu_loop) = PDU_STORAGE.try_split().expect("can only split once");

    let client = Arc::new(Client::new(
        pdu_loop,
        Timeouts {
            wait_loop_delay: Duration::from_millis(2),
            mailbox_response: Duration::from_millis(1000),
            ..Default::default()
        },
        ClientConfig::default(),
    ));

    ex.spawn(tx_rx_task(&interface, tx, rx).unwrap()).detach();

    let group = SlaveGroup::<MAX_SLAVES, PDI_LEN>::new(|slave| {
        Box::pin(async {
            // EL3004 needs some specific config during init to function properly
            if slave.name() == "EL3004" {
                log::info!("Found EL3004. Configuring...");

                slave.write_sdo(0x1c12, SubIndex::Index(0), 0u8).await?;
                slave.write_sdo(0x1c13, SubIndex::Index(0), 0u8).await?;

                slave
                    .write_sdo(0x1c13, SubIndex::Index(1), 0x1a00u16)
                    .await?;
                slave
                    .write_sdo(0x1c13, SubIndex::Index(2), 0x1a02u16)
                    .await?;
                slave
                    .write_sdo(0x1c13, SubIndex::Index(3), 0x1a04u16)
                    .await?;
                slave
                    .write_sdo(0x1c13, SubIndex::Index(4), 0x1a06u16)
                    .await?;
                slave.write_sdo(0x1c13, SubIndex::Index(0), 4u8).await?;
            }

            Ok(())
        })
    });

    let group = client
        // Initialise up to 16 slave devices
        .init::<MAX_SLAVES, _>(group, |groups, slave| groups.push(slave))
        .await
        .expect("Init");

    log::info!("Group has {} slaves", group.len());

    for slave in group.slaves() {
        let (i, o) = slave.io();

        log::info!(
            "-> Slave {} {} has {} input bytes, {} output bytes",
            slave.configured_address,
            slave.name,
            i.len(),
            o.len(),
        );
    }

    let mut tick_interval = Timer::interval(Duration::from_millis(5));

    let group = Arc::new(group);
    let group2 = group.clone();

    while let Some(_) = tick_interval.next().await {
        group.tx_rx(&client).await.expect("TX/RX");

        // Increment every output byte for every slave device by one
        for slave in group2.slaves() {
            let (_i, o) = slave.io();

            for byte in o.iter_mut() {
                *byte = byte.wrapping_add(1);
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    env_logger::init();
    let local_ex = LocalExecutor::new();

    let ctrlc = CtrlC::new().expect("cannot create Ctrl+C handler?");

    futures_lite::future::block_on(
        local_ex.run(ctrlc.race(async { main_inner(&local_ex).await.unwrap() })),
    );

    Ok(())
}
