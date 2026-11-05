// SPDX-License-Identifier: GPL-3.0-or-later

//! The script methods of the GUI objects: what every object answers, and what each kind adds.

use super::SkinRuntime;
use crate::draw::Rect;
use crate::gui::{ButtonKind, Kind, ObjId};
use crate::maki::{Handle, Value};

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

impl SkinRuntime {
    /// A method of GUI object `o`; `None` for one the object does not answer.
    pub(crate) fn gui_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        if let Some(v) = self.kind_maki_call(o, name, args) {
            return Some(v);
        }
        if self.objects[o].group.is_some() {
            if let Some(v) = self.group_maki_call(o, name, args) {
                return Some(v);
            }
        }
        if let Some(v) = self.base_maki_call(o, name, args) {
            return Some(v);
        }
        // a group passes what it has no use for to the child it exposes
        if self.objects[o].group.is_some() {
            if let Some(e) = self.embedded(o) {
                return self.gui_maki_call(e, name, args);
            }
        }
        None
    }

    /// The methods a kind adds before the group's and the base's.
    fn kind_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        match &self.objects[o].kind {
            Kind::Layout(_) => self.layout_maki_call(o, name, args),
            Kind::Layer(_) => match name {
                "isinvalid" => Some(Value::Bool(false)),
                _ => None,
            },
            Kind::AnimatedLayer(..) => self.anim_maki_call(o, name, args).or(match name {
                "isinvalid" => Some(Value::Bool(false)),
                _ => None,
            }),
            Kind::Button(_) => self.button_maki_call(o, name, args),
            Kind::Text(_) => self.text_maki_call(o, name, args),
            Kind::Slider(_) => match name {
                "setposition" => {
                    self.slider_set_position(o, arg(args, 0).as_int(), false);
                    Some(Value::Null)
                }
                "getposition" => Some(Value::Int(match &self.objects[o].kind {
                    Kind::Slider(s) => s.position,
                    _ => 0,
                })),
                "lock" | "unlock" => {
                    if let Kind::Slider(s) = &mut self.objects[o].kind {
                        s.locked = name == "lock";
                    }
                    Some(Value::Null)
                }
                _ => None,
            },
            Kind::Frame(_) => self.frame_maki_call(o, name, args),
            Kind::Bucket(_) => self.bucket_maki_call(o, name, args),
            Kind::Vis(_) => self.vis_maki_call(o, name, args),
            _ => None,
        }
    }

    fn layout_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        Some(match name {
            "getcontainer" => Value::of(
                self.objects[o]
                    .layout()
                    .and_then(|d| d.container)
                    .map(Handle::Container),
            ),
            "getscale" => Value::Num(1.0),
            "setscale" | "beforeredock" | "redock" | "setdesktopalpha" | "center" => Value::Null,
            "snapadjust" => {
                self.snap_adjust(
                    o,
                    arg(args, 0).as_int(),
                    arg(args, 1).as_int(),
                    arg(args, 2).as_int(),
                    arg(args, 3).as_int(),
                );
                Value::Null
            }
            "getdesktopalpha" => Value::Bool(false),
            "resize" => {
                let to = Rect::new(
                    arg(args, 0).as_int(),
                    arg(args, 1).as_int(),
                    arg(args, 2).as_int(),
                    arg(args, 3).as_int(),
                );
                if !self.maximizes(o, to) {
                    self.layout_move_to(o, to.left, to.top, to.width, to.height);
                    self.resize_layout_to(o, to.width, to.height);
                }
                Value::Null
            }
            "getleft" => Value::Int(self.objects[o].layout().map(|d| d.view_x).unwrap_or(0)),
            "gettop" => Value::Int(self.objects[o].layout().map(|d| d.view_y).unwrap_or(0)),
            "getsnapadjustleft" => {
                Value::Int(self.objects[o].layout().map(|d| d.snap_left).unwrap_or(0))
            }
            "getsnapadjusttop" => {
                Value::Int(self.objects[o].layout().map(|d| d.snap_top).unwrap_or(0))
            }
            "getsnapadjustright" => {
                Value::Int(self.objects[o].layout().map(|d| d.snap_right).unwrap_or(0))
            }
            "getsnapadjustbottom" => {
                Value::Int(self.objects[o].layout().map(|d| d.snap_bottom).unwrap_or(0))
            }
            _ => return None,
        })
    }

    /// Whether a script's resize of `layout` to `to` is a maximize, which a phone does not do:
    /// the main window set over the whole viewport. The window stays as it is. A maximize a tap
    /// asks for shows the app's fullscreen visualizer instead (VIS_FS); one a script repeats by
    /// itself does nothing more.
    pub(crate) fn maximizes(&mut self, layout: ObjId, to: Rect) -> bool {
        let (vw, vh) = self.viewport();
        let covers = to.left <= 0 && to.top <= 0 && to.right() >= vw && to.bottom() >= vh;
        if Some(layout) != self.main_layout() || !covers {
            return false;
        }
        if self.firing_now() == Some("onleftclick") && self.host_actions {
            self.host.host_action("VIS_FS", None);
        }
        true
    }

    fn anim_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        let a = match &self.objects[o].kind {
            Kind::AnimatedLayer(_, a) => a.clone(),
            _ => return None,
        };
        Some(match name {
            "setspeed" => {
                if let Kind::AnimatedLayer(_, a) = &mut self.objects[o].kind {
                    a.speed = arg(args, 0).as_int();
                }
                Value::Null
            }
            "play" => {
                self.anim_play(o);
                Value::Null
            }
            // pause again resumes
            "pause" => {
                if a.playing {
                    let now = self.now();
                    let paused = !a.paused;
                    if let Kind::AnimatedLayer(_, a) = &mut self.objects[o].kind {
                        a.paused = paused;
                        if !paused {
                            a.next_at = now + a.speed.max(1) as i64;
                        }
                    }
                    self.dispatch(
                        Handle::Gui(o),
                        if paused { "onpause" } else { "onresume" },
                        &[],
                    );
                }
                Value::Null
            }
            "stop" => {
                self.anim_stop(o);
                Value::Null
            }
            "setautoreplay" => {
                if let Kind::AnimatedLayer(_, a) = &mut self.objects[o].kind {
                    a.auto_replay = arg(args, 0).truthy();
                }
                Value::Null
            }
            "setrealtime" => Value::Null,
            "gotoframe" => {
                self.anim_goto_frame(o, arg(args, 0).as_int());
                Value::Null
            }
            "getcurframe" => Value::Int(a.frame),
            "getlength" => Value::Int(self.anim_length(o)),
            "getstartframe" => Value::Int(a.start_frame),
            "getendframe" => Value::Int(a.end_frame.unwrap_or_else(|| self.anim_length(o) - 1)),
            "setstartframe" => {
                if let Kind::AnimatedLayer(_, a) = &mut self.objects[o].kind {
                    a.start_frame = arg(args, 0).as_int();
                }
                Value::Null
            }
            "setendframe" => {
                if let Kind::AnimatedLayer(_, a) = &mut self.objects[o].kind {
                    a.end_frame = Some(arg(args, 0).as_int());
                }
                Value::Null
            }
            "isplaying" => Value::Bool(a.playing && !a.paused),
            "ispaused" => Value::Bool(a.playing && a.paused),
            "isstopped" => Value::Bool(!a.playing),
            "getautoreplay" => Value::Bool(a.auto_replay),
            "getdirection" => Value::Int((self.anim_vertical(o) == Some(true)) as i32),
            _ => return None,
        })
    }

    fn button_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        let kind = match &self.objects[o].kind {
            Kind::Button(b) => b.kind.clone(),
            _ => return None,
        };
        if name == "getcurcfgval" {
            return Some(match kind {
                ButtonKind::Toggle { .. } => Value::Int(self.button_is_active(o) as i32),
                ButtonKind::NStates { .. } => Value::Int(self.button_cfg_value(o)),
                ButtonKind::Plain => return None,
            });
        }
        Some(match name {
            "leftclick" => {
                self.button_click(o);
                Value::Null
            }
            "rightclick" => {
                self.dispatch(Handle::Gui(o), "onrightclick", &[]);
                Value::Null
            }
            // tells the scripts (onActivate), unlike setActivatedNoCallback; Bento's tabs take their
            // active look from it
            "setactivated" => {
                let on = arg(args, 0).truthy();
                if let Kind::Button(b) = &mut self.objects[o].kind {
                    b.activated = on;
                }
                self.repaint();
                self.dispatch(Handle::Gui(o), "onactivate", &[Value::Int(on as i32)]);
                Value::Null
            }
            "getactivated" => Value::Bool(self.button_is_active(o)),
            "setactivatednocallback" => {
                let on = arg(args, 0).truthy();
                if let Kind::Button(b) = &mut self.objects[o].kind {
                    b.activated = on;
                }
                self.repaint();
                Value::Null
            }
            _ => return None,
        })
    }

    fn group_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        Some(match name {
            "getobject" => {
                let wanted = arg(args, 0).as_string();
                let found = self.find_object(o, &wanted);
                if found.is_none() {
                    let me = self.object_text(o);
                    self.host.log(&format!("getobject miss: {me} -> {wanted}"));
                }
                Value::of(found.map(Handle::Gui))
            }
            // a xui tab button's activation is its embedded button's (suicore.maki activates Bento's tabs)
            "setactivated" => match self.embedded(o) {
                Some(e) => self.gui_maki_call(e, name, args).unwrap_or(Value::Null),
                None => Value::Null,
            },
            "getactivated" => match self.embedded(o) {
                Some(e) => self
                    .gui_maki_call(e, name, args)
                    .unwrap_or(Value::Bool(false)),
                None => Value::Bool(false),
            },
            "getnumobjects" | "getnumchildren" => {
                Value::Int(self.objects[o].children().len() as i32)
            }
            "enumchildren" | "enumobject" => Value::of(
                usize::try_from(arg(args, 0).as_int())
                    .ok()
                    .and_then(|i| self.objects[o].children().get(i).copied())
                    .map(Handle::Gui),
            ),
            "init" => {
                if let Some(Handle::Gui(g)) = arg(args, 0).as_object() {
                    if self.objects[g].group.is_some() {
                        self.init_into(o, g);
                    }
                }
                Value::Null
            }
            // the last pointer, relative to this group
            "getmouseposx" => Value::Int(self.last_pointer.0 - self.objects[o].base.rect.left),
            "getmouseposy" => Value::Int(self.last_pointer.1 - self.objects[o].base.rect.top),
            "islayout" => Value::Bool(self.objects[o].is_layout()),
            "getparentlayout" => Value::of(self.layout_object(o).map(Handle::Gui)),
            "lockminmaxsize" | "unlockminmaxsize" => Value::Null,
            "getxmlparam" => {
                let k = arg(args, 0).as_string().to_lowercase();
                let own = self.objects[o].base.attr(&k).map(|s| s.to_string());
                let v = own.or_else(|| {
                    self.embedded(o)
                        .and_then(|e| self.objects[e].base.attr(&k).map(|s| s.to_string()))
                });
                Value::string(v.unwrap_or_default())
            }
            _ => return None,
        })
    }

    /// `tag#id`, as the reference engine names an object in a log line.
    pub(crate) fn object_text(&self, o: ObjId) -> String {
        format!("{}#{}", self.objects[o].base.tag, self.objects[o].base.id)
    }

    fn base_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        Some(match name {
            "getclassname" => Value::string(self.objects[o].class_name().to_string()),
            "getid" => Value::string(self.objects[o].base.id.clone()),
            "show" => {
                self.show_object(o);
                Value::Null
            }
            "hide" => {
                self.hide_object(o);
                Value::Null
            }
            "isvisible" => Value::Bool(self.objects[o].base.visible),
            "getleft" => Value::Int(self.laid_out(o).left),
            "gettop" => Value::Int(self.laid_out(o).top),
            "getwidth" => Value::Int(self.laid_out(o).width),
            "getheight" => Value::Int(self.laid_out(o).height),
            "getguix" => Value::Int(self.objects[o].base.x.unwrap_or(0)),
            "getguiy" => Value::Int(self.objects[o].base.y.unwrap_or(0)),
            "getguiw" => Value::Int(self.objects[o].base.w.unwrap_or(0)),
            "getguih" => Value::Int(self.objects[o].base.h.unwrap_or(0)),
            "getguirelatx" => Value::Int(self.objects[o].base.relat_x),
            "getguirelaty" => Value::Int(self.objects[o].base.relat_y),
            "getguirelatw" => Value::Int(self.objects[o].base.relat_w),
            "getguirelath" => Value::Int(self.objects[o].base.relat_h),
            "getautowidth" => Value::Int(
                self.intrinsic_size(o)
                    .map(|s| s.0)
                    .unwrap_or(self.objects[o].base.rect.width),
            ),
            "getautoheight" => Value::Int(
                self.intrinsic_size(o)
                    .map(|s| s.1)
                    .unwrap_or(self.objects[o].base.rect.height),
            ),
            "resize" => {
                let b = &mut self.objects[o].base;
                b.x = Some(arg(args, 0).as_int());
                b.y = Some(arg(args, 1).as_int());
                b.w = Some(arg(args, 2).as_int());
                b.h = Some(arg(args, 3).as_int());
                b.relat_x = 0;
                b.relat_y = 0;
                b.relat_w = 0;
                b.relat_h = 0;
                self.relayout();
                Value::Null
            }
            "setxmlparam" => {
                let k = arg(args, 0).as_string().to_lowercase();
                let v = arg(args, 1).as_string();
                self.set_xml_param(o, &k, &v);
                Value::Null
            }
            "getxmlparam" => Value::string(
                self.objects[o]
                    .base
                    .attr(&arg(args, 0).as_string().to_lowercase())
                    .unwrap_or("")
                    .to_string(),
            ),
            "setalpha" => {
                let paint = self.objects[o].base.paint_key();
                let was_zero = self.objects[o].base.alpha() == 0;
                self.objects[o]
                    .base
                    .set_alpha(arg(args, 0).as_int().clamp(0, 255));
                if self.objects[o].base.paint_key() != paint {
                    self.alpha_changed(o, was_zero);
                }
                Value::Null
            }
            "getalpha" => Value::Int(self.objects[o].base.alpha()),
            "getparent" => Value::of(self.objects[o].base.parent.map(Handle::Gui)),
            "getparentlayout" => Value::of(self.layout_object(o).map(Handle::Gui)),
            "findobject" => {
                let wanted = arg(args, 0).as_string();
                let found = self.find_object_chain(o, &wanted);
                if found.is_none() {
                    let me = self.object_text(o);
                    self.host.log(&format!("findobject miss: {me} -> {wanted}"));
                }
                Value::of(found.map(Handle::Gui))
            }
            "findobjectxy" => Value::of(
                self.hit_test_stack(arg(args, 0).as_int(), arg(args, 1).as_int())
                    .map(Handle::Gui),
            ),
            "getname" => Value::string(
                self.objects[o]
                    .base
                    .attr("name")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| self.objects[o].base.id.clone()),
            ),
            "bringtofront" | "bringtoback" | "setfocus" | "invalidate" | "repaint"
            | "setcursor" | "setenabled" | "setcapture" | "releasecapture" | "setregion"
            | "setregionfrommap" | "fx_setenabled" | "fx_setwrap" | "fx_setrect" | "fx_setbgfx"
            | "fx_setclear" | "fx_setspeed" | "fx_setlocalized" | "fx_setbilinear"
            | "fx_setalphamode" | "fx_setgridsize" | "fx_update" | "fx_restart" | "fx_setsize" => {
                if name == "setenabled" && self.objects[o].base.enabled != arg(args, 0).truthy() {
                    let on = arg(args, 0).truthy();
                    self.objects[o].base.enabled = on;
                    // scripts hear it (Wasabi SDK guiobjwnd)
                    self.dispatch(Handle::Gui(o), "onenable", &[Value::Int(on as i32)]);
                }
                Value::Null
            }
            "getenabled" => Value::Bool(self.objects[o].base.enabled),
            "settargetx" | "settargety" | "settargetw" | "settargeth" | "settargeta" => {
                let i = match name {
                    "settargetx" => 0,
                    "settargety" => 1,
                    "settargetw" => 2,
                    "settargeth" => 3,
                    _ => 4,
                };
                let v = arg(args, 0).as_int();
                self.objects[o].base.target[i] = Some(if i == 4 { v.clamp(0, 255) } else { v });
                Value::Null
            }
            "settargetspeed" => {
                self.objects[o].base.target_seconds = arg(args, 0).as_double();
                Value::Null
            }
            "gototarget" => {
                self.go_to_target(o);
                Value::Null
            }
            "isgoingtotarget" => Value::Bool(self.objects[o].base.move_start.is_some()),
            "canceltarget" => {
                self.cancel_target(o);
                Value::Null
            }
            "clienttoscreenx" | "clienttoscreenw" | "screentoclientx" | "screentoclientw"
            | "clienttoscreeny" | "clienttoscreenh" | "screentoclienty" | "screentoclienth" => {
                arg(args, 0)
            }
            // the object hears the onAction; a plain object answers 1 (Wasabi SDK basewnd, guiobjwnd)
            "sendaction" => {
                let mut a: Vec<Value> = (0..6).map(|i| arg(args, i)).collect();
                a.push(Value::Obj(Handle::Gui(o)));
                self.dispatch(Handle::Gui(o), "onaction", &a);
                Value::Int(1)
            }
            "istransparencysafe" | "islayoutanimationsafe" => Value::Bool(true),
            "getguid" => Value::str(""),
            "ismouseoverrect" => Value::Bool(false),
            "isactive" => Value::Bool(true),
            _ => return None,
        })
    }

    /// The rectangle after any pending layout, for scripts that read it right after a change.
    fn laid_out(&mut self, o: ObjId) -> Rect {
        self.ensure_layout();
        self.objects[o].base.rect
    }

    fn frame_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        match name {
            "getposition" => Some(Value::Int(self.frame_position(o))),
            "setposition" => {
                self.frame_set_position(o, arg(args, 0).as_int());
                self.relayout();
                Some(Value::Null)
            }
            _ => None,
        }
    }

    fn bucket_maki_call(&mut self, o: ObjId, name: &str, args: &[Value]) -> Option<Value> {
        match name {
            "setscroll" => {
                let before = self.bucket_scroll(o);
                self.bucket_scroll_to(o, arg(args, 0).as_int());
                Some(Value::Int(before))
            }
            "getscroll" => Some(Value::Int(self.bucket_scroll(o))),
            "getmaxwidth" => Some(Value::Int(self.bucket_max(o, true))),
            "getmaxheight" => Some(Value::Int(self.bucket_max(o, false))),
            _ => None,
        }
    }
}
