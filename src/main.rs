#![deny(clippy::undocumented_unsafe_blocks)]
// These create a lot of noise until no stubs are left.
#![allow(dead_code)]

use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, TryRecvError},
    },
};

slint::include_modules!();
mod sameboy;
use sameboy::{DebuggerCmdStr, SameBoy, Schedule};
use slint::{Model, SharedString, VecModel};

fn main() {
    let main_window = MainWindow::new().expect("Unable to create main window");
    let (debugger_sender, debugger_receiver) = std::sync::mpsc::channel();
    let debugger_window: Rc<DebuggerWindow> =
        Rc::new(DebuggerWindow::new().expect("Unable to create debugger window"));
    let log_model = Rc::new(VecModel::from(vec![(
        debugger_window.get_out_color(),
        SharedString::new(),
    )]));
    debugger_window.set_log(log_model.clone().into());
    let sameboy = Arc::new(Mutex::new(SameBoy::new(
        main_window.as_weak(),
        debugger_receiver,
        debugger_window.as_weak(),
    )));

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
        main_window.on_show_debugger({
            let debugger_window = Rc::clone(&debugger_window);
            move || {
                debugger_window
                    .show()
                    .expect("Unable to show debugger window");
            }
        });
        debugger_window.on_submit({
            let debugger_window = Rc::clone(&debugger_window);
            let debugger_sender = debugger_sender.clone();
            move || {
                let color = debugger_window.get_cmd_color();

                let command = debugger_window.get_command();
                let debugger_command = DebuggerCmdStr::new(&command);
                if log_model
                    .row_data(log_model.row_count() - 1)
                    .is_some_and(|row| row.1.is_empty())
                {
                    log_model.set_row_data(log_model.row_count() - 1, (color, command));
                } else {
                    log_model.push((color, command));
                }
                log_model.push((debugger_window.get_out_color(), SharedString::new()));
                // It's okay if the other end has hung up.
                let _ = debugger_sender.send(debugger_command);
                debugger_window.set_command(SharedString::new());
            }
        });
        main_window.run().expect("Error running application");

        // Important to do this before the scope implicitly attempts to join the thread,
        // otherwise we deadlock.
        let _ = debugger_sender.send(DebuggerCmdStr::new_null()); // Resumes emulation if it was paused in the debugger.
        let _ = sender.send(Schedule::Quit); // Tell the emulation thread to shut down.
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
