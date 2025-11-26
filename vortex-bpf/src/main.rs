#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::xdp_action,
    macros::{xdp, map},
    programs::XdpContext,
    maps::HashMap,
};
use aya_log_ebpf::info;

#[map]
static mut PACKET_COUNTS: HashMap<u32, u64> = HashMap::with_max_entries(1024, 0);

#[xdp]
pub fn vortex_xdp(ctx: XdpContext) -> u32 {
    match try_vortex_xdp(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_vortex_xdp(ctx: XdpContext) -> Result<u32, u32> {
    // Simple packet counting
    // Key 0: Total packets
    let key = 0;
    
    unsafe {
        if let Some(count) = PACKET_COUNTS.get(&key) {
            let new_count = count + 1;
            PACKET_COUNTS.insert(&key, &new_count, 0).map_err(|_| xdp_action::XDP_ABORTED)?;
            
            // Log every 1000 packets to avoid flooding
            if new_count % 1000 == 0 {
                info!(&ctx, "Packet count: {}", new_count);
            }
        } else {
            PACKET_COUNTS.insert(&key, &1, 0).map_err(|_| xdp_action::XDP_ABORTED)?;
        }
    }

    Ok(xdp_action::XDP_PASS)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
