#![deny(clippy::undocumented_unsafe_blocks)]
// These create a lot of noise until no stubs are left.
#![allow(dead_code)]

use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, TryRecvError},
};

slint::include_modules!();
mod sameboy;
use sameboy::{SameBoy, Schedule};

fn main() {
    let main_window = MainWindow::new().expect("Unable to create main window");
    let sameboy = Arc::new(Mutex::new(SameBoy::new(main_window.as_weak())));

    let (sender, receiver) = std::sync::mpsc::sync_channel(0);
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("GB emulation".into())
            .spawn_scoped(scope, {
                let sameboy = &sameboy;
                move || emu_thread_func(sameboy, receiver)
            })
            .expect("Unable to spawn emulation thread");

        main_window.on_load({
            let sameboy = Arc::clone(&sameboy);
            let sender = sender.clone();
            move || {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Game Boy ROM", &["gb", "gbc"])
                    .pick_file()
                {
                    // If the thread is dead, the locking will fail anyway.
                    let _ = sender.send(Schedule::Stop);
                    sameboy.lock().unwrap().load_rom(&path);
                    // Ditto.
                    let _ = sender.send(Schedule::Run);
                }
            }
        });
        main_window.on_quit(|| {
            let _ = slint::quit_event_loop();
        });
        main_window.run().expect("Error running application");

        // Important to do this before the scope implicitly attempts to join the thread,
        // as hanging the channel up is what causes the emulation thread to shut down.
        let _ = sender.send(Schedule::Quit);
        drop(sender);
    });

    // TODO: save application state and all that
}

fn emu_thread_func(sameboy: &Arc<Mutex<SameBoy>>, receiver: Receiver<Schedule>) {
    while let Ok(mut schedule) = receiver.recv() {
        'schedule: loop {
            match schedule {
                Schedule::Stop => {}
                Schedule::Run => {
                    let mut sameboy = sameboy.lock().unwrap();
                    loop {
                        match receiver.try_recv() {
                            Err(TryRecvError::Disconnected) => return,
                            Err(TryRecvError::Empty) => {}
                            Ok(new_schedule) => {
                                schedule = new_schedule;
                                continue 'schedule;
                            }
                        }
                        sameboy.run_once();
                    }
                }
                Schedule::Step => {
                    let mut sameboy = sameboy.lock().unwrap();
                    sameboy.step();
                }
                Schedule::RunFrame => {
                    let mut sameboy = sameboy.lock().unwrap();
                    todo!();
                }

                Schedule::Quit => return,
            }
            break;
        }
    }
}

const MAIN_WINDOW_NAME: &str = "aythya";
