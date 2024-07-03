use egui::FontDefinitions;
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use eframe::egui;
use subprocess::Exec;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let font_name = "NotoSansSC-Regular";
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        font_name.to_owned(),
        egui::FontData::from_static(include_bytes!("../NotoSansSC-Regular.otf")),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, font_name.to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(font_name.to_owned());

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };

    // Our application state:
    let mut cmd = "".to_owned();
    let mut out_s = "".to_owned();

    let (tx, rx): (Sender<String>, Receiver<String>) = channel();

    eframe::run_simple_native("Rclone GUI", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.set_fonts(fonts.clone());
            ui.heading("Rclone GUI");
            ui.horizontal(|ui| {
                let cmd_label = ui.label("Your command: ");
                ui.text_edit_singleline(&mut cmd).labelled_by(cmd_label.id);
            });
            if ui.button("Run").clicked() {
                "".clone_into(&mut out_s);
                let cmd = cmd.clone();
                let tx_clone = tx.clone();
                // 在新线程中执行命令并异步发送输出
                thread::spawn(move || {
                    let mut r_out = Exec::shell(cmd).stream_stdout().expect("");
                    let mut buffer = [0; 1024];
                    loop {
                        match r_out.read(&mut buffer) {
                            Ok(n) if n > 0 => {
                                let output = String::from_utf8(buffer[..n].to_vec()).expect("");
                                tx_clone.send(output).unwrap();
                            }
                            Ok(0) => break, // 子进程结束
                            Err(e) => {
                                println!("Error reading output: {}", e);
                                break;
                            }
                            Ok(1_usize..) => {}
                        }
                    }
                });
            };
            ui.label(format!("run: '{cmd}'"));
            ui.add(egui::TextEdit::multiline(&mut out_s));
            // 从接收器中读取输出并更新 TextEdit
            for output in rx.try_iter() {
                out_s.push_str(&output);
            }
        });
    })
}
