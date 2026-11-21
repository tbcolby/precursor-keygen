//! Key Ceremony: TRNG-backed password, PIN, passphrase, BIP-39 mnemonic and
//! key-material generator for Precursor.
//!
//! This file only routes messages. Generators and the screen state machine
//! live in `keygen-core` (host-tested); `app.rs` draws and talks to the PDDB.

mod app;

use num_traits::{FromPrimitive, ToPrimitive};
use precursor_appkit::{app as kit, keys, pump, store};

/// xous-names server name. Any unique string; NOT the GAM app name.
const SERVER_NAME: &str = "_Key Ceremony_";

#[derive(Debug, num_derive::FromPrimitive, num_derive::ToPrimitive)]
pub(crate) enum AppOp {
    Redraw = 0,
    Rawkeys,
    FocusChange,
    Tick,
    BasisChanged,
    Quit,
}

/// The 1 s tick only runs while a secret is on screen (for the auto-hide
/// timeout) and the app is in front.
fn sync_pump(pump: &pump::Pump, focused: bool, app: &app::App) {
    if focused && app.needs_tick() {
        pump.run();
    } else {
        pump.stop();
    }
}

fn main() -> ! {
    log_server::init_wait().unwrap();
    log::set_max_level(log::LevelFilter::Info);
    log::info!("keygen PID is {}", xous::process::id());

    let xns = xous_names::XousNames::new().unwrap();
    let sid = xns.register_name(SERVER_NAME, None).expect("can't register server");

    // GAM app name comes from apps/manifest.json key "keygen".
    let reg = kit::register(
        &xns,
        sid,
        gam::APP_NAME_KEYGEN,
        gam::UxType::Framebuffer,
        kit::Opcodes {
            redraw: AppOp::Redraw.to_u32().unwrap(),
            rawkeys: Some(AppOp::Rawkeys.to_u32().unwrap()),
            focus_change: Some(AppOp::FocusChange.to_u32().unwrap()),
            got_input: None,
        },
    );

    let store = store::Store::open(app::DICT);
    store::watch_bases(xous::connect(sid).unwrap(), AppOp::BasisChanged.to_usize().unwrap());
    let pump = pump::Pump::spawn(xous::connect(sid).unwrap(), AppOp::Tick.to_usize().unwrap(), 1000);

    let mut app = app::App::new(&xns, reg.screen, store);
    let mut focused = false;

    loop {
        let msg = xous::receive_message(sid).unwrap();
        match FromPrimitive::from_usize(msg.body.id()) {
            Some(AppOp::Redraw) => app.draw(),
            Some(AppOp::Rawkeys) => xous::msg_scalar_unpack!(msg, k1, k2, k3, k4, {
                for key in keys::decode_rawkeys([k1, k2, k3, k4]) {
                    if app.on_key(key) == app::Action::Leave {
                        kit::leave(app.screen());
                    }
                }
                app.draw();
                sync_pump(&pump, focused, &app);
            }),
            Some(AppOp::FocusChange) => xous::msg_scalar_unpack!(msg, code, _, _, _, {
                match kit::focus_state(code) {
                    gam::FocusState::Foreground => {
                        focused = true;
                        app.on_foreground();
                    }
                    gam::FocusState::Background => {
                        focused = false;
                        app.on_background();
                    }
                }
                sync_pump(&pump, focused, &app);
            }),
            Some(AppOp::Tick) => {
                app.on_tick();
                pump::ack(&msg);
                sync_pump(&pump, focused, &app);
            }
            Some(AppOp::BasisChanged) => app.reload(),
            Some(AppOp::Quit) => break,
            None => log::warn!("unknown opcode {:?}", msg.body.id()),
        }
    }
    pump.quit();
    kit::shutdown(&xns, sid)
}
