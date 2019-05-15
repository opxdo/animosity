use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gio::prelude::*;
use gtk;
use gtk::prelude::*;

use crate::anim;
use crate::combo_box_enum::ComboBoxEnum;
use crate::frame_import;
use crate::frame_info::{FrameInfo, parse_frame_info};
use crate::int_entry::{IntSize, IntEntry};
use crate::select_dir;
use crate::{label_section, lookup_action, error_msg_box, info_msg_box, SpriteInfo, SpriteType};

pub fn frame_import_dialog(sprite_info: &Arc<SpriteInfo>, parent: &gtk::ApplicationWindow) {
    let tex_id = sprite_info.tex_id();
    let mut files = match sprite_info.files.try_borrow_mut() {
        Ok(o) => o,
        _ => return,
    };
    let layer_names;
    let tex_formats;
    let is_anim;
    let path;
    let grp_scale;
    {
        let file = match files.file(tex_id.0, tex_id.1) {
            Ok(Some(o)) => o,
            _ => return,
        };
        is_anim = file.is_anim();
        layer_names = file.layer_names().into_owned();
        tex_formats = file.texture_formats();
        path = file.path().to_owned();
        grp_scale = file.grp().map(|x| x.scale);
    }
    let has_hd2 = if is_anim && tex_id.1 != SpriteType::Sd {
        let other_ty = match tex_id.1 {
            SpriteType::Hd => SpriteType::Hd2,
            SpriteType::Hd2 | _ => SpriteType::Hd,
        };
        match files.file(tex_id.0, other_ty) {
            Ok(Some(_)) => true,
            _ => false,
        }
    } else {
        false
    };

    let window = gtk::Window::new(gtk::WindowType::Toplevel);

    let mut framedef_filename = None;
    let framedef_status = gtk::Label::new(None);
    framedef_status.set_halign(gtk::Align::Start);
    let framedef;
    let mut hd2_framedef_filename = None;
    let mut hd2_framedef = None;
    let mut hd2_framedef_bx = None;
    let hd2_framedef_status = gtk::Label::new(None);
    hd2_framedef_status.set_halign(gtk::Align::Start);
    let framedef_bx = if is_anim {
        framedef = Rc::new(
            select_dir::SelectFile::new(&window, "import_frames", "Text files", "*.json")
        );
        let framedef_inner_bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        framedef_inner_bx.pack_start(&framedef.widget(), false, false, 0);
        framedef_inner_bx.pack_start(&framedef_status, false, false, 5);
        framedef_filename = framedef.text().and_then(|x| match x.is_empty() {
            true => None,
            false => Some(x),
        });
        if has_hd2 {
            let hd2_framedef_ = Rc::new(select_dir::SelectFile::new(
                &window, "import_frames_hd2", "Text files", "*.json"
            ));
            let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
            bx.pack_start(&hd2_framedef_.widget(), false, false, 0);
            bx.pack_start(&hd2_framedef_status, false, false, 5);
            hd2_framedef_filename = hd2_framedef_.text().and_then(|x| match x.is_empty() {
                true => None,
                false => Some(x),
            });
            hd2_framedef = Some(hd2_framedef_);
            hd2_framedef_bx = Some(label_section("HD2 Frame info file", &bx));
            label_section("HD Frame info file", &framedef_inner_bx)
        } else {
            label_section("Frame info file", &framedef_inner_bx)
        }
    } else {
        framedef = Rc::new(
            select_dir::SelectFile::new(&window, "import_frames_grp", "PNG files", "*.png")
        );
        let inner_bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner_bx.pack_start(&framedef.widget(), false, false, 0);
        label_section("First frame", &inner_bx)
    };

    let mut checkboxes = Vec::with_capacity(layer_names.len());
    let mut grp_format = None;
    static FORMATS: &[(anim::TextureFormat, &str)] = &[
        (anim::TextureFormat::Dxt1, "DXT1"),
        (anim::TextureFormat::Dxt5, "DXT5"),
        (anim::TextureFormat::Monochrome, "Monochrome"),
    ];
    let layers_bx = if is_anim {
        let grid = gtk::Grid::new();
        grid.set_column_spacing(5);
        grid.set_row_spacing(5);

        for (i, name) in layer_names.iter().enumerate() {
            let row = i as i32 + 1;

            let checkbox = gtk::CheckButton::new();
            grid.attach(&checkbox, 0, row, 1, 1);
            checkbox.set_sensitive(false);

            let label = gtk::Label::new(Some(&**name));
            grid.attach(&label, 1, row, 1, 1);
            label.set_halign(gtk::Align::Start);
            let format = ComboBoxEnum::new(FORMATS);
            grid.attach(format.widget(), 2, row, 1, 1);

            checkboxes.push((checkbox, format));
        }
        label_section("Layers", &grid)
    } else {
        let format = ComboBoxEnum::new(FORMATS);
        if let Some(Ok(Some(tex_f))) = tex_formats.get(0) {
            format.set_active(tex_f);
        }

        let bx = label_section("Encode format", format.widget());
        grp_format = Some(format);
        bx
    };
    let grp_scale_entry;
    let grp_scale_bx;
    if is_anim {
        grp_scale_entry = None;
        grp_scale_bx = None;
    } else {
        let entry = IntEntry::new(IntSize::Int8);
        entry.set_value(grp_scale.unwrap_or(0).into());
        grp_scale_bx = Some(label_section("Scale", &entry.frame));
        grp_scale_entry = Some(entry);
    };

    let button_bx = gtk::Box::new(gtk::Orientation::Horizontal, 15);
    let ok_button = gtk::Button::new_with_label("Import");
    ok_button.set_sensitive(!is_anim);
    let cancel_button = gtk::Button::new_with_label("Cancel");
    let w = window.clone();
    cancel_button.connect_clicked(move |_| {
        w.destroy();
    });
    let sprite_info = sprite_info.clone();
    let w = window.clone();
    let frame_info: Rc<RefCell<Option<FrameInfo>>> = Rc::new(RefCell::new(None));
    let hd2_frame_info: Rc<RefCell<Option<FrameInfo>>> = Rc::new(RefCell::new(None));
    let fi = frame_info.clone();
    let hd2_fi = hd2_frame_info.clone();
    let fi_entry = framedef.clone();
    let hd2_fi_entry = hd2_framedef.clone();
    let checkboxes = Rc::new(checkboxes);
    let checkboxes2 = checkboxes.clone();
    ok_button.connect_clicked(move |_| {
        // Used for grps
        let filename_prefix;
        let dir = match fi_entry.text() {
            Some(s) => {
                let mut buf: PathBuf = s.into();
                filename_prefix = match buf.file_name() {
                    Some(s) => {
                        let text = s.to_string_lossy();
                        match text.find("_000.") {
                            Some(s) => Some((&text[..s]).to_string()),
                            None => None,
                        }
                    }
                    None => None,
                };
                buf.pop();
                if !buf.is_dir() {
                    return;
                }
                buf
            }
            None => return,
        };
        let hd2_dir = hd2_fi_entry.as_ref().and_then(|x| x.text()).and_then(|s| {
            let mut buf: PathBuf = s.into();
            buf.pop();
            if !buf.is_dir() {
                None
            } else {
                Some(buf)
            }
        });
        let mut files = sprite_info.files.borrow_mut();
        let result = if is_anim {
            let formats = checkboxes2.iter().map(|x| {
                Ok(match x.1.get_active() {
                    Some(x) => x,
                    None => {
                        if x.0.get_active() {
                            return Err(());
                        }
                        // Just a dummy value since the layer is unused
                        anim::TextureFormat::Monochrome
                    }
                })
            }).collect::<Result<Vec<_>, ()>>();
            let formats = match formats {
                Ok(o) => o,
                Err(()) => {
                    error_msg_box(&w, "Format not specified for every layer");
                    return;
                }
            };
            let fi = fi.borrow();
            let frame_info = match *fi {
                Some(ref s) => s,
                None => return,
            };
            let hd2_fi = hd2_fi.borrow();

            let result = frame_import::import_frames(
                &mut files,
                frame_info,
                hd2_fi.as_ref(),
                &dir,
                hd2_dir.as_ref().map(|x| &**x),
                &formats,
                tex_id.0,
                tex_id.1,
            );
            let frame_count = if hd2_fi.is_some() { 2 } else { 1 } *
                frame_info.layers.len() as u32 * frame_info.frame_count;
            result.map(|()| frame_count)
        } else {
            let format = match grp_format {
                Some(ref s) => s.get_active(),
                None => return,
            };
            let format = match format {
                Some(o) => o,
                None => {
                    error_msg_box(&w, "Format not specified");
                    return;
                }
            };
            let filename_prefix = match filename_prefix {
                Some(o) => o,
                None => {
                    error_msg_box(&w, "Invalid first frame filename");
                    return;
                }
            };
            let scale = grp_scale_entry.as_ref().unwrap().get_value() as u8;
            let result = frame_import::import_frames_grp(
                &mut files,
                &dir,
                &filename_prefix,
                format,
                tex_id.0,
                scale,
            );
            result
        };
        match result {
            Ok(frame_count) => {
                sprite_info.draw_clear_all();
                if let Ok(mut file) = files.file(tex_id.0, tex_id.1) {
                    sprite_info.changed_ty(tex_id, &mut file);
                }
                drop(files);
                if let Some(a) = lookup_action(&sprite_info.sprite_actions, "is_dirty") {
                    a.activate(Some(&true.to_variant()));
                }

                info_msg_box(&w, format!("Imported {} frames", frame_count));
                w.destroy();
            }
            Err(e) => {
                drop(files);
                use std::fmt::Write;
                let mut msg = format!("Unable to import frames:\n");
                for c in e.iter_chain() {
                    writeln!(msg, "{}", c).unwrap();
                }
                // Remove last newline
                msg.pop();
                error_msg_box(&w, msg);
            }
        }
    });

    let ok = ok_button.clone();
    if is_anim {
        // The second entry is used for hd2
        let framedef_set = Rc::new(move |filename: &str, hd2: bool, status: &gtk::Label| {
            match parse_frame_info(Path::new(filename)) {
                Ok(o) => {
                    ok.set_sensitive(true);
                    for &(ref check, ref format) in checkboxes.iter() {
                        check.set_active(false);
                        format.set_sensitive(false);
                        format.clear_active();
                    }
                    for &(i, _) in &o.layers {
                        if let Some(&(ref check, ref format)) = checkboxes.get(i as usize) {
                            check.set_active(true);
                            format.set_sensitive(true);
                            let tex_f = tex_formats.get(i as usize)
                                .and_then(|x| x.as_ref().ok())
                                .and_then(|x| x.as_ref());
                            if let Some(tex_f) = tex_f {
                                format.set_active(tex_f);
                            }
                        }
                    }
                    if hd2 {
                        *hd2_frame_info.borrow_mut() = Some(o);
                    } else {
                        *frame_info.borrow_mut() = Some(o);
                    }
                    status.set_text("");
                }
                Err(e) => {
                    ok.set_sensitive(false);
                    for &(ref check, ref format) in checkboxes.iter() {
                        check.set_active(false);
                        format.set_sensitive(false);
                        format.clear_active();
                    }
                    let mut msg = format!("Frame info invalid:\n");
                    for c in e.iter_chain() {
                        use std::fmt::Write;
                        writeln!(msg, "{}", c).unwrap();
                    }
                    // Remove last newline
                    msg.pop();
                    if hd2 {
                        *hd2_frame_info.borrow_mut() = None;
                    } else {
                        *frame_info.borrow_mut() = None;
                    }
                    status.set_text(&msg);
                }
            }
        });
        if let Some(filename) = framedef_filename {
            framedef_set(&filename, false, &framedef_status);
        }
        let fun = framedef_set.clone();
        framedef.on_change(move |filename| {
            fun(filename, false, &framedef_status);
        });
        if let Some(filename) = hd2_framedef_filename {
            framedef_set(&filename, true, &hd2_framedef_status);
        }
        if let Some(ref fdef) = hd2_framedef {
            let fun = framedef_set.clone();
            fdef.on_change(move |filename| {
                fun(filename, true, &hd2_framedef_status);
            });
        }
    }

    button_bx.pack_end(&cancel_button, false, false, 0);
    button_bx.pack_end(&ok_button, false, false, 0);
    let bx = gtk::Box::new(gtk::Orientation::Vertical, 10);
    bx.pack_start(&framedef_bx, false, false, 0);
    if let Some(hd2) = hd2_framedef_bx {
        bx.pack_start(&hd2, false, false, 0);
    }
    bx.pack_start(&layers_bx, false, false, 0);
    if let Some(scale) = grp_scale_bx {
        bx.pack_start(&scale, false, false, 0);
    }
    bx.pack_start(&button_bx, false, false, 0);
    window.add(&bx);
    window.set_border_width(10);
    window.set_property_default_width(350);
    if is_anim {
        window.set_title(&format!("Import frames for {:?} image {}", tex_id.1, tex_id.0));
    } else {
        if let Some(filename) = path.file_name() {
            window.set_title(&format!("Import frames of {}", filename.to_string_lossy()));
        }
    }
    window.set_modal(true);
    window.set_transient_for(Some(parent));
    window.show_all();
}
