use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
};

use egui::{Color32, RichText};
use mmwave_awr::AwrDescriptor;
use mmwave_core::{
    config::Configuration,
    devices::{DeviceConfig, EmptyDeviceDescriptor},
    message::Id,
    transform::Transform,
};
use mmwave_playback::{PlaybackDescriptor};
use mmwave_recorder::RecordingDescriptor;
use mmwave_zed::ZedDescriptor;
use tracing::info;

#[derive(Default)]
pub struct ConfigWidget {
    pub colors: HashMap<Id, [f32; 3]>,
    pub config: Configuration,
    pub config_original: Configuration,
    pub inbound_config: Option<Configuration>,
    pub outbound_config: Option<Configuration>,
}

impl ConfigWidget {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.render_header(ui);
        ui.separator();
        self.render_descriptors(ui);
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.render_reload_config_button(ui);
            self.render_apply_config_button(ui);
            self.render_save_config_button(ui);
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("new awr").clicked() {
                self.config.descriptors.push(DeviceConfig {
                    id: Id::Device(0, 0),
                    device_descriptor: Box::new(AwrDescriptor::default()),
                });
            }
            if ui.button("new recorder").clicked() {
                self.config.descriptors.push(DeviceConfig {
                    id: Id::Device(0, 0),
                    device_descriptor: Box::new(RecordingDescriptor::default()),
                });
            }
            if ui.button("new zed").clicked() {
                self.config.descriptors.push(DeviceConfig {
                    id: Id::Device(0, 0),
                    device_descriptor: Box::new(ZedDescriptor::default()),
                });
            }
            if ui.button("new playback").clicked() {
                self.config.descriptors.push(DeviceConfig {
                    id: Id::Device(0, 0),
                    device_descriptor: Box::new(PlaybackDescriptor::default()),
                });
            }
            if ui.button("new empty").clicked() {
                self.config.descriptors.push(DeviceConfig {
                    id: Id::Device(0, 0),
                    device_descriptor: Box::new(EmptyDeviceDescriptor),
                });
            }
        });
    }

    fn render_reload_config_button(&mut self, ui: &mut egui::Ui) {
        let config_changed = bincode::serialize(&Some(self.config.clone())).ok()
            != bincode::serialize(&self.inbound_config).ok();
        let config_available = self.inbound_config.is_some() && config_changed;
        let reload_config = ui
            .add_enabled_ui(config_available, |ui| {
                if config_available {
                    ui.button(RichText::new("Use remote config").color(Color32::RED))
                } else {
                    ui.button(RichText::new("Config up to date").color(Color32::GREEN))
                }
            })
            .inner;

        if config_available && reload_config.clicked() {
            if let Some(new_config) = &mut self.inbound_config.take() {
                std::mem::swap(&mut self.config, new_config);
                self.config_original = self.config.clone();
            }
        }
    }

    fn render_apply_config_button(&mut self, ui: &mut egui::Ui) {
        let config_changed = self.is_config_changed();
        let apply_config = ui
            .add_enabled_ui(config_changed, |ui| ui.button("Apply Config"))
            .inner;

        if config_changed && apply_config.clicked() {
            self.config_original = self.config.clone();
            self.outbound_config = Some(self.config.clone());
        }
    }

    fn render_save_config_button(&self, ui: &mut egui::Ui) {
        if ui.button("save to config_out.json").clicked() {
            let mut file = File::create("config_out.json").unwrap();

            let Ok(config) = serde_json::to_string_pretty(&self.config) else {
                return;
            };

            let _ = file.write_all(config.as_bytes());
        }
    }

    fn is_config_changed(&self) -> bool {
        bincode::serialize(&self.config).ok() != bincode::serialize(&self.config_original).ok()
    }

    fn render_descriptors(&mut self, ui: &mut egui::Ui) {
        let mut removals = Vec::new();
        ui.centered_and_justified(|ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, descriptor) in self.config.descriptors.iter_mut().enumerate() {
                    let combo_id = ui.make_persistent_id(format!("device_type_combo_{}", i));
                    egui::CollapsingHeader::new(descriptor.title())
                        .id_source(combo_id)
                        .show(ui, |ui| {
                            if ui.button("delete").clicked() {
                                removals.push(i);
                            } else {
                                Self::render_descriptor_color_edit(
                                    &mut self.colors,
                                    ui,
                                    descriptor.id,
                                );
                                descriptor.ui(ui);
                            }
                        });
                }
            });
        });

        removals.sort();
        for i in removals.iter().rev() {
            self.config.descriptors.swap_remove(*i);
        }
    }

    fn render_descriptor_color_edit(
        colors: &mut HashMap<Id, [f32; 3]>,
        ui: &mut egui::Ui,
        descriptor_id: Id,
    ) {
        let color = colors.entry(descriptor_id).or_insert([1.0, 1.0, 1.0]);
        ui.horizontal(|ui| {
            ui.label("Color:");
            ui.color_edit_button_rgb(color);
        });
    }
}
