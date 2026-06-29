use crate::AppWindow;
use crate::SceneEventHandler;
use crate::bind_callbacks;
use crate::messages::{AppMessage, FileIOMessage};
use cad_core::{CadFileManager, IdaphsPrtFormat};
use cad_renderer::*;
use rfd::FileDialog;
use slint::Weak;

pub fn setup_menubar_controls(
    ui_weak: Weak<AppWindow>,
    sender: smol::channel::Sender<ControlMessage<AppMessage>>,
) {
    let Some(ui) = ui_weak.upgrade() else {
        eprintln!("Failed to upgrade Weak reference to AppWindow");
        return;
    };
    bind_callbacks!(ui, sender, {
        on_new_file => AppMessage::FileIOMsg { msg: FileIOMessage::NewFile },
        on_open_file => AppMessage::FileIOMsg { msg: FileIOMessage::OpenFile },
        on_export_file => AppMessage::FileIOMsg { msg: FileIOMessage::ExportFile },

    });
}

impl SceneEventHandler {
    pub fn handle_menubar_event(&mut self, msg: FileIOMessage) -> bool {
        use FileIOMessage::*;
        match msg {
            NewFile => false,
            OpenFile => {
                let file_path_maybe = FileDialog::new()
                    .add_filter("Idaphs Part File", &["idaphsprt"])
                    .set_directory(".")
                    .pick_file();

                if let Some(file_path) = file_path_maybe {
                    let cad_data =
                        CadFileManager::load_part_from_file(file_path, IdaphsPrtFormat).unwrap();

                    // overwrite the current model with the loaded cad_data
                    let mut model = self.ctx.write_model();
                    *model = cad_data;
                    println!("Open file requested");

                    true
                } else {
                    false
                }
            }
            ExportFile => {
                let model = self.ctx.read_model();
                let file_path_maybe = FileDialog::new()
                    .add_filter("Idaphs Part File", &["idaphsprt"])
                    .set_directory(".")
                    .save_file();

                if let Some(file_path) = file_path_maybe {
                    CadFileManager::save_part_to_file(file_path, &model, IdaphsPrtFormat).unwrap();
                } else {
                    println!("Export file canceled");
                }

                false
            }
        }
    }
}
