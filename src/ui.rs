//! The window: a setup screen (server, registration keys, updates) and the
//! kiosk screen (camera, who was recognised, recent check-ins, sync status).

use crate::client::Client;
use crate::i18n::{long_date, t};
use crate::state::{Mode, Seen, Shared, UpdateState};
use crate::store::{Config, Event, EventStatus, EventType, Gallery, Identity, Lang};
use crate::style::{Palette as P, tabular};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, ClipboardItem, Context, Div, Entity, FocusHandle, FontWeight,
    Hsla, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, RenderImage,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, actions,
    div, img, linear_color_stop, linear_gradient, px, svg,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

actions!(kiosk, [OpenSetup, ToggleFullscreen]);

#[derive(PartialEq)]
enum Screen {
    Setup,
    Kiosk,
}

pub struct KioskApp {
    shared: Arc<Shared>,
    client: Arc<Client>,
    identity: Identity,
    config: Config,
    models: PathBuf,
    screen: Screen,
    focus: FocusHandle,
    server: Entity<InputState>,
    camera: Entity<InputState>,
    preview: Option<Arc<RenderImage>>,
    connecting: bool,
    setup_message: Option<(bool, String)>,
    copied: Option<(&'static str, Instant)>,
}

impl KioskApp {
    pub fn new(
        shared: Arc<Shared>,
        client: Arc<Client>,
        identity: Identity,
        config: Config,
        models: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let server = cx.new(|cx| {
            let mut s =
                InputState::new(window, cx).placeholder(t(config.lang, "setup.server.hint"));
            s.set_value(config.server_url.clone(), window, cx);
            s
        });
        let camera = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("0");
            s.set_value(config.camera_index.to_string(), window, cx);
            s
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let screen = if config.is_configured() && shared.gallery.lock().is_some() {
            Screen::Kiosk
        } else {
            Screen::Setup
        };

        // ~15 fps: swap in the newest preview, free the old texture, repaint.
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(66))
                    .await;
                let alive = this.update_in(cx, |this, window, cx| {
                    if let Some(next) = this.shared.take_preview()
                        && let Some(old) = this.preview.replace(next)
                    {
                        let _ = window.drop_image(old);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
            shared,
            client,
            identity,
            config,
            models,
            screen,
            focus,
            server,
            camera,
            preview: None,
            connecting: false,
            setup_message: None,
            copied: None,
        }
    }

    fn lang(&self) -> Lang {
        self.config.lang
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.connecting {
            return;
        }
        let mut url = self
            .server
            .read(cx)
            .value()
            .trim()
            .trim_end_matches('/')
            .to_string();
        if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
            url = format!("https://{url}");
            self.server
                .update(cx, |s, cx| s.set_value(url.clone(), window, cx));
        }
        self.config.server_url = url.clone();
        self.config.camera_index = self.camera.read(cx).value().trim().parse().unwrap_or(0);
        if let Err(e) = self.config.save() {
            self.setup_message = Some((false, format!("{e:#}")));
            cx.notify();
            return;
        }
        self.client.set_base(&url);
        self.connecting = true;
        self.setup_message = None;
        cx.notify();
        let (shared, client, models) = (
            self.shared.clone(),
            self.client.clone(),
            self.models.clone(),
        );
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    crate::face::models::ensure(&models, &url)?;
                    crate::sync::pull_once(&shared, &client)?;
                    shared.wake_sync();
                    anyhow::Ok(())
                })
                .await;
            this.update(cx, |this, cx| {
                this.connecting = false;
                match result {
                    Ok(()) => {
                        this.setup_message = Some((true, t(this.lang(), "setup.ok").into()));
                        this.screen = Screen::Kiosk;
                    }
                    Err(e) => this.setup_message = Some((false, format!("{e:#}"))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_lang(&mut self, lang: Lang, cx: &mut Context<Self>) {
        self.config.lang = lang;
        let _ = self.config.save();
        cx.notify();
    }

    fn was_copied(&self, id: &str) -> bool {
        self.copied
            .is_some_and(|(c, at)| c == id && at.elapsed() < Duration::from_millis(1600))
    }

    // ---------------------------------------------------------------- setup

    fn render_setup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let connected = self.shared.gallery.lock().is_some();
        let has_server = !self.server.read(cx).value().trim().is_empty();

        // Which step the person is on: the first one not done yet.
        let done = [has_server, connected, connected];
        let current = done.iter().position(|d| !d).unwrap_or(3);
        let steps = v_flex().gap_5().children(
            [
                ("setup.step1", "setup.step1.hint"),
                ("setup.step2", "setup.step2.hint"),
                ("setup.step3", "setup.step3.hint"),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (title, hint))| {
                step(i + 1, t(lang, title), t(lang, hint), done[i], i == current)
            }),
        );

        let brand = v_flex()
            .w(px(340.))
            .flex_none()
            .p_8()
            .gap_10()
            .border_r_1()
            .border_color(P::line())
            .bg(linear_gradient(
                160.,
                linear_color_stop(P::accent().alpha(0.10), 0.),
                linear_color_stop(P::accent().alpha(0.0), 0.75),
            ))
            .child(
                h_flex().gap_3().items_center().child(mark(40.)).child(
                    v_flex()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Voltrus Kiosk"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(P::muted())
                                .child(t(lang, "brand.tagline")),
                        ),
                ),
            )
            .child(steps)
            .child(div().flex_1())
            .child(
                v_flex()
                    .gap_2()
                    .child(caption(t(lang, "setup.shortcuts")))
                    .child(shortcut("Ctrl Shift S", t(lang, "setup.shortcut.setup")))
                    .child(shortcut("F11", t(lang, "setup.shortcut.full"))),
            );

        let lang_switch = segmented(
            34.,
            [
                ("lang-id", None, "Indonesia", lang == Lang::Id),
                ("lang-en", None, "English", lang == Lang::En),
            ],
            |i| if i == 0 { Lang::Id } else { Lang::En },
            cx.listener(|this, value: &Lang, _, cx| this.set_lang(*value, cx)),
        );

        let uid = self.identity.device_uid.clone();
        let key = self.identity.public_key_b64();
        let identity = v_flex()
            .gap_2()
            .child(caption(t(lang, "setup.identity")))
            .child(
                v_flex()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(P::line())
                    .bg(P::bg())
                    .child(self.identity_row("copy-uid", "hash", t(lang, "setup.uid"), uid, cx))
                    .child(div().h(px(1.)).bg(P::line()))
                    .child(self.identity_row(
                        "copy-key",
                        "key-round",
                        t(lang, "setup.key"),
                        key,
                        cx,
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(P::muted())
                    .child(t(lang, "setup.register")),
            );

        let message = self.setup_message.clone().map(|(ok, text)| {
            let color = if ok { P::accent() } else { P::danger() };
            h_flex()
                .gap_3()
                .items_start()
                .p_3()
                .rounded(px(12.))
                .bg(color.alpha(0.08))
                .border_1()
                .border_color(color.alpha(0.3))
                .child(icon(
                    if ok { "circle-check" } else { "circle-alert" },
                    18.,
                    color,
                ))
                .child(div().flex_1().min_w_0().text_sm().child(text))
        });

        let actions = h_flex()
            .gap_3()
            .child(
                Button::new("connect")
                    .primary()
                    .large()
                    .label(if self.connecting {
                        t(lang, "setup.connecting")
                    } else {
                        t(lang, "setup.connect")
                    })
                    .disabled(self.connecting)
                    .on_click(cx.listener(|this, _, window, cx| this.connect(window, cx))),
            )
            .when(connected, |el| {
                el.child(
                    Button::new("back")
                        .large()
                        .label(t(lang, "setup.back"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.screen = Screen::Kiosk;
                            cx.notify();
                        })),
                )
            });

        let form = v_flex()
            .flex_1()
            .min_w_0()
            .p_8()
            .gap_6()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(lang, "setup.title")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(P::muted())
                            .child(t(lang, "setup.subtitle")),
                    ),
            )
            .child(field(
                t(lang, "setup.server"),
                Input::new(&self.server)
                    .large()
                    .prefix(icon("globe", 16., P::muted()))
                    .into_any_element(),
            ))
            .child(
                h_flex()
                    .gap_4()
                    .items_end()
                    .child(
                        div().w(px(140.)).child(field(
                            t(lang, "setup.camera"),
                            Input::new(&self.camera)
                                .large()
                                .prefix(icon("camera", 16., P::muted()))
                                .into_any_element(),
                        )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(field(t(lang, "setup.lang"), lang_switch.into_any_element())),
                    ),
            )
            .child(identity)
            .children(message)
            .child(actions)
            .child(self.render_update_row(cx));

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_8()
            .child(
                h_flex()
                    .items_stretch()
                    .w(px(1000.))
                    .max_w_full()
                    .max_h_full()
                    .overflow_hidden()
                    .rounded(px(24.))
                    .border_1()
                    .border_color(P::line_strong())
                    .bg(P::surface())
                    .shadow_2xl()
                    .child(brand)
                    .child(form),
            )
            .into_any_element()
    }

    fn identity_row(
        &self,
        id: &'static str,
        icon_name: &'static str,
        label: &'static str,
        value: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lang = self.lang();
        let copied = self.was_copied(id);
        let copy = value.clone();
        h_flex()
            .gap_3()
            .px_4()
            .py_3()
            .items_center()
            .child(icon(icon_name, 16., P::dim()))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_xs().text_color(P::muted()).child(label))
                    .child(
                        div()
                            .text_sm()
                            .font_family("monospace")
                            .truncate()
                            .child(value),
                    ),
            )
            .child(
                h_flex()
                    .id(id)
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if copied { P::accent() } else { P::muted() })
                    .bg(P::raised())
                    .hover(|s| s.text_color(P::text()))
                    .child(icon(
                        if copied { "check" } else { "copy" },
                        13.,
                        if copied { P::accent() } else { P::muted() },
                    ))
                    .child(if copied {
                        t(lang, "setup.copied")
                    } else {
                        t(lang, "setup.copy")
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                        this.copied = Some((id, Instant::now()));
                        cx.notify();
                    })),
            )
    }

    /// Version, update state, auto-update switch, "check now" / "restart now".
    fn render_update_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let state = self.shared.update.lock().clone();
        let (text, color) = match &state {
            UpdateState::Idle => (String::new(), P::muted()),
            UpdateState::Checking => (t(lang, "update.checking").to_string(), P::muted()),
            UpdateState::UpToDate => (t(lang, "update.current").to_string(), P::muted()),
            UpdateState::Ready(v) => (format!("{} {v}", t(lang, "update.ready")), P::accent()),
            UpdateState::Failed(e) => (format!("{}: {e}", t(lang, "update.failed")), P::danger()),
        };
        let action = if matches!(state, UpdateState::Ready(_)) {
            Button::new("update-restart")
                .primary()
                .small()
                .label(t(lang, "update.restart"))
                .on_click(cx.listener(|this, _, _, _| crate::update_loop::ask(&this.shared)))
        } else {
            Button::new("update-check")
                .small()
                .label(t(lang, "update.check"))
                .disabled(state == UpdateState::Checking)
                .on_click(cx.listener(|this, _, _, cx| {
                    crate::update_loop::ask(&this.shared);
                    *this.shared.update.lock() = UpdateState::Checking;
                    cx.notify();
                }))
        };
        h_flex()
            .flex_wrap()
            .gap_x_4()
            .gap_y_3()
            .pt_5()
            .border_t_1()
            .border_color(P::line())
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(icon("refresh-cw", 14., P::muted()))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(t(lang, "setup.update")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(P::dim())
                                    .child(format!("v{}", crate::updater::VERSION)),
                            ),
                    )
                    .when(!text.is_empty(), |el| {
                        el.child(div().text_xs().text_color(color).truncate().child(text))
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Switch::new("update-auto")
                            .checked(self.config.auto_update)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.config.auto_update = *checked;
                                let _ = this.config.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(P::muted())
                            .child(t(lang, "update.auto")),
                    ),
            )
            .child(action)
            .into_any_element()
    }

    // ---------------------------------------------------------------- kiosk

    fn render_kiosk(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let gallery = self.shared.gallery.lock().clone();
        let (seen, since) = self.shared.seen.lock().clone();
        let mode = *self.shared.mode.lock();
        let sync = self.shared.sync.lock().clone();
        let (queued, recent) = {
            let events = self.shared.events.lock();
            (
                events.queue.len(),
                events
                    .log
                    .iter()
                    .rev()
                    .take(12)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        };
        let now = chrono::Local::now();

        let place = gallery
            .as_ref()
            .map(|g| {
                [g.device.name.clone(), g.device.location.clone()]
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default();
        let header = h_flex()
            .justify_between()
            .items_center()
            .child(
                h_flex().gap_4().items_center().child(mark(48.)).child(
                    v_flex()
                        .gap_0p5()
                        .child(
                            div().text_xl().font_weight(FontWeight::SEMIBOLD).child(
                                gallery
                                    .as_ref()
                                    .and_then(|g| g.company.clone())
                                    .unwrap_or_else(|| t(lang, "title").into()),
                            ),
                        )
                        .when(!place.is_empty(), |el| {
                            el.child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .text_sm()
                                    .text_color(P::muted())
                                    .child(icon("map-pin", 14., P::muted()))
                                    .child(place),
                            )
                        }),
                ),
            )
            .child(
                v_flex()
                    .items_end()
                    .child(
                        h_flex()
                            .items_baseline()
                            .gap_1()
                            .font_features(tabular())
                            .child(
                                div()
                                    .text_size(px(44.))
                                    .line_height(px(48.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(now.format("%H:%M").to_string()),
                            )
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(P::dim())
                                    .child(now.format("%S").to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(P::muted())
                            .child(long_date(lang, now.date_naive())),
                    ),
            );

        let body = h_flex()
            .flex_1()
            .min_h_0()
            .gap_5()
            .child(self.render_stage(lang, &seen, gallery.as_ref()))
            .child(
                v_flex()
                    .w(px(400.))
                    .flex_none()
                    .h_full()
                    .gap_4()
                    .child(segmented(
                        42.,
                        [
                            (
                                "mode-auto",
                                Some("arrow-left-right"),
                                t(lang, "mode.auto"),
                                mode == Mode::Auto,
                            ),
                            (
                                "mode-in",
                                Some("log-in"),
                                t(lang, "mode.in"),
                                mode == Mode::In,
                            ),
                            (
                                "mode-out",
                                Some("log-out"),
                                t(lang, "mode.out"),
                                mode == Mode::Out,
                            ),
                        ],
                        |i| [Mode::Auto, Mode::In, Mode::Out][i],
                        cx.listener(|this, value: &Mode, _, cx| {
                            *this.shared.mode.lock() = *value;
                            cx.notify();
                        }),
                    ))
                    .child(status_card(lang, &seen, since, gallery.as_ref()))
                    .child(recent_card(lang, recent)),
            );

        let engine_error = self.shared.engine_error.lock().clone();
        // Offline is expected and already said; show only real errors.
        let error = if sync.online {
            sync.error.clone()
        } else {
            None
        }
        .or(engine_error);
        let (dot, state_text) = if sync.online {
            (P::accent(), t(lang, "status.online"))
        } else {
            (P::warn(), t(lang, "status.offline"))
        };
        let footer = h_flex()
            .justify_between()
            .items_center()
            .gap_4()
            .text_xs()
            .text_color(P::muted())
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_full()
                            .bg(dot.alpha(0.18))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(px(6.)).rounded_full().bg(dot)),
                    )
                    .child(state_text)
                    .when_some(error, |el, e| {
                        el.child(div().min_w_0().truncate().text_color(P::danger()).child(e))
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_none()
                    .when_some(gallery.as_ref(), |el, g| {
                        el.child(chip(
                            "users",
                            format!("{} {}", g.employees.len(), t(lang, "status.people")),
                            P::muted(),
                        ))
                        .child(chip(
                            "scan-face",
                            format!("{} {}", g.faces.len(), t(lang, "status.faces")),
                            P::muted(),
                        ))
                    })
                    .child(chip(
                        "upload",
                        format!("{queued} {}", t(lang, "status.queue")),
                        if queued > 0 { P::warn() } else { P::muted() },
                    ))
                    .child(
                        div()
                            .pl_1()
                            .text_color(P::dim())
                            .child(format!("v{}", crate::updater::VERSION)),
                    ),
            );

        v_flex()
            .size_full()
            .px_8()
            .pt_6()
            .pb_5()
            .gap_6()
            .child(header)
            .child(body)
            .child(footer)
            .into_any_element()
    }

    fn render_stage(&self, lang: Lang, seen: &Seen, gallery: Option<&Gallery>) -> impl IntoElement {
        let camera_error = self.shared.camera_error();
        let ring = match seen {
            Seen::Recorded { event } if event.payload.event_type == EventType::Out => P::out(),
            Seen::Recorded { .. } => P::accent(),
            Seen::NotLive => P::danger(),
            _ => P::line_strong(),
        };
        let show_hint = *seen == Seen::Nobody && gallery.is_some_and(|g| !g.faces.is_empty());
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(px(24.))
            .overflow_hidden()
            .bg(gpui_kit::black())
            .border_2()
            .border_color(ring)
            .map(|el| match (&self.preview, &camera_error) {
                (Some(p), _) => el.child(img(p.clone()).size_full().object_fit(ObjectFit::Cover)),
                (None, Some(e)) => el.child(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .p_8()
                        .child(
                            div()
                                .size(px(72.))
                                .rounded_full()
                                .bg(P::raised())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("camera-off", 32., P::muted())),
                        )
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t(lang, "status.camera")),
                        )
                        .child(div().text_xs().text_color(P::dim()).child(e.clone())),
                ),
                (None, None) => el,
            })
            .when(self.preview.is_some(), |el| {
                el.child(
                    h_flex()
                        .absolute()
                        .top_4()
                        .left_4()
                        .gap_2()
                        .items_center()
                        .px_3()
                        .py_1p5()
                        .rounded_full()
                        .bg(gpui_kit::black().alpha(0.55))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(div().size(px(7.)).rounded_full().bg(P::danger()))
                        .child(t(lang, "live")),
                )
            })
            .when(show_hint && self.preview.is_some(), |el| {
                el.child(
                    h_flex()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(120.))
                        .items_end()
                        .justify_center()
                        .pb_6()
                        .bg(linear_gradient(
                            180.,
                            linear_color_stop(gpui_kit::black().alpha(0.0), 0.),
                            linear_color_stop(gpui_kit::black().alpha(0.7), 1.),
                        ))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .px_4()
                                .py_2()
                                .rounded_full()
                                .bg(gpui_kit::black().alpha(0.5))
                                .border_1()
                                .border_color(P::line_strong())
                                .text_sm()
                                .child(icon("scan-face", 16., P::text()))
                                .child(t(lang, "seen.nobody.hint")),
                        ),
                )
            })
    }
}

// -------------------------------------------------------------- pieces

fn icon(name: &str, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

fn mark(size: f32) -> impl IntoElement {
    img("brand/mark.png")
        .size(px(size))
        .flex_none()
        .rounded(px(size * 0.23))
}

fn caption(text: &'static str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(P::muted())
        .child(text)
}

fn field(label: &'static str, body: AnyElement) -> impl IntoElement {
    v_flex().gap_2().child(caption(label)).child(body)
}

fn chip(icon_name: &'static str, text: String, color: Hsla) -> impl IntoElement {
    h_flex()
        .gap_1p5()
        .items_center()
        .px_2p5()
        .py_1()
        .rounded_full()
        .bg(P::surface())
        .border_1()
        .border_color(P::line())
        .text_color(color)
        .child(icon(icon_name, 12., color))
        .child(text)
}

fn shortcut(keys: &'static str, label: &'static str) -> impl IntoElement {
    h_flex()
        .gap_3()
        .items_center()
        .child(h_flex().gap_1().children(keys.split(' ').map(|k| {
            div()
                .px_1p5()
                .min_w(px(22.))
                .flex()
                .justify_center()
                .rounded(px(6.))
                .bg(P::raised())
                .border_1()
                .border_color(P::line_strong())
                .text_xs()
                .font_family("monospace")
                .text_color(P::text())
                .child(k)
        })))
        .child(div().text_xs().text_color(P::muted()).child(label))
}

fn step(
    n: usize,
    title: &'static str,
    hint: &'static str,
    done: bool,
    current: bool,
) -> impl IntoElement {
    let badge = div()
        .size(px(28.))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .map(|el| {
            if done {
                el.bg(P::accent()).child(icon("check", 14., P::accent_fg()))
            } else if current {
                el.border_1()
                    .border_color(P::accent())
                    .text_color(P::accent())
                    .child(n.to_string())
            } else {
                el.border_1()
                    .border_color(P::line_strong())
                    .text_color(P::dim())
                    .child(n.to_string())
            }
        });
    h_flex().gap_3().items_start().child(badge).child(
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_0p5()
            .pt(px(3.))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if done || current {
                        P::text()
                    } else {
                        P::muted()
                    })
                    .child(title),
            )
            .child(div().text_xs().text_color(P::muted()).child(hint)),
    )
}

/// A row of pill buttons, one selected. `value(i)` maps the index to what
/// `on_pick` receives.
fn segmented<T: 'static, const N: usize>(
    height: f32,
    items: [(&'static str, Option<&'static str>, &'static str, bool); N],
    value: impl Fn(usize) -> T,
    on_pick: impl Fn(&T, &mut Window, &mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    let on_pick = std::rc::Rc::new(on_pick);
    h_flex()
        .p_1()
        .gap_1()
        .rounded(px(14.))
        .bg(P::surface())
        .border_1()
        .border_color(P::line())
        .children(
            items
                .into_iter()
                .enumerate()
                .map(|(i, (id, icon_name, label, selected))| {
                    let picked = value(i);
                    let on_pick = on_pick.clone();
                    h_flex()
                        .id(id)
                        .flex_1()
                        .h(px(height))
                        .gap_2()
                        .items_center()
                        .justify_center()
                        .rounded(px(10.))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .map(|el| {
                            if selected {
                                el.bg(P::raised())
                                    .border_1()
                                    .border_color(P::line_strong())
                                    .text_color(P::text())
                            } else {
                                el.text_color(P::muted()).hover(|s| s.text_color(P::text()))
                            }
                        })
                        .when_some(icon_name, |el, name| {
                            el.child(icon(
                                name,
                                15.,
                                if selected { P::accent() } else { P::dim() },
                            ))
                        })
                        .child(label)
                        .on_click(move |_, window, cx| on_pick(&picked, window, cx))
                }),
        )
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

fn avatar(name: &str, size: f32, color: Hsla) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(color.alpha(0.14))
        .border_1()
        .border_color(color.alpha(0.4))
        .text_size(px(size * 0.36))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(initials(name))
}

fn event_color(event: &Event) -> Hsla {
    if event.payload.event_type == EventType::In {
        P::accent()
    } else {
        P::out()
    }
}

fn status_card(
    lang: Lang,
    seen: &Seen,
    since: Instant,
    gallery: Option<&Gallery>,
) -> impl IntoElement {
    let card = |tone: Hsla| {
        v_flex()
            .relative()
            .gap_4()
            .p_6()
            .min_h(px(250.))
            .justify_center()
            .rounded(px(20.))
            .overflow_hidden()
            .bg(P::surface())
            .border_1()
            .border_color(tone.alpha(0.3))
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(tone.alpha(0.10), 0.),
                linear_color_stop(tone.alpha(0.0), 0.8),
            )))
    };
    let message = |icon_name: &'static str, tone: Hsla, title: String, hint: &'static str| {
        card(tone)
            .child(
                div()
                    .size(px(60.))
                    .rounded(px(18.))
                    .bg(tone.alpha(0.12))
                    .border_1()
                    .border_color(tone.alpha(0.25))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(icon_name, 28., tone)),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(px(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when(!hint.is_empty(), |el| {
                        el.child(div().text_base().text_color(P::muted()).child(hint))
                    }),
            )
            .into_any_element()
    };

    match (gallery, seen) {
        (None, _) => message(
            "unplug",
            P::muted(),
            t(lang, "seen.nogallery").into(),
            t(lang, "status.nogallery"),
        ),
        (Some(_), Seen::Recorded { event }) => {
            let tone = event_color(event);
            let is_in = event.payload.event_type == EventType::In;
            let position = gallery
                .and_then(|g| {
                    g.employees
                        .iter()
                        .find(|e| e.id == event.payload.employee_id)
                })
                .and_then(|e| e.position.clone());
            card(tone)
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(avatar(&event.name, 72., tone))
                        .child(
                            v_flex()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(tone)
                                        .child(t(
                                            lang,
                                            if is_in { "recorded.in" } else { "recorded.out" },
                                        )),
                                )
                                .child(
                                    div()
                                        .text_size(px(28.))
                                        .line_height(px(34.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(event.name.clone()),
                                )
                                .when_some(position, |el, p| {
                                    el.child(div().text_sm().text_color(P::muted()).child(p))
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .self_start()
                        .px_3()
                        .py_1p5()
                        .rounded_full()
                        .bg(tone.alpha(0.14))
                        .text_color(tone)
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .font_features(tabular())
                        .child(icon(if is_in { "log-in" } else { "log-out" }, 15., tone))
                        .child(format!(
                            "{} · {}",
                            t(lang, if is_in { "mode.in" } else { "mode.out" }),
                            local_time(&event.payload.ts)
                        )),
                )
                .into_any_element()
        }
        (Some(_), Seen::Again { name }) => card(P::muted())
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(avatar(name, 64., P::muted()))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name.clone()),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .text_sm()
                                    .text_color(P::muted())
                                    .child(icon("check-check", 15., P::muted()))
                                    .child(t(lang, "seen.again.hint")),
                            ),
                    ),
            )
            .into_any_element(),
        (Some(_), Seen::NotLive) => message(
            "shield-alert",
            P::danger(),
            t(lang, "seen.notlive").into(),
            t(lang, "seen.notlive.hint"),
        ),
        (Some(g), _) if g.faces.is_empty() => message(
            "users",
            P::warn(),
            t(lang, "seen.nofaces").into(),
            t(lang, "seen.nofaces.hint"),
        ),
        (Some(_), Seen::Matching) => message(
            "scan-face",
            P::accent(),
            t(lang, "seen.matching").into(),
            t(lang, "seen.matching.hint"),
        ),
        // A face that has matched nobody for a moment, not a single miss.
        (Some(_), Seen::Unknown) if since.elapsed() < Duration::from_millis(1500) => message(
            "scan-face",
            P::accent(),
            t(lang, "seen.matching").into(),
            t(lang, "seen.matching.hint"),
        ),
        (Some(_), Seen::Unknown) => message(
            "user-x",
            P::warn(),
            t(lang, "seen.unknown").into(),
            t(lang, "seen.unknown.hint"),
        ),
        (Some(_), Seen::Nobody) => {
            message("scan-face", P::text(), t(lang, "seen.nobody").into(), "")
        }
    }
}

fn recent_card(lang: Lang, recent: Vec<Event>) -> impl IntoElement {
    let count = recent.len();
    v_flex()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .rounded(px(20.))
        .bg(P::surface())
        .border_1()
        .border_color(P::line())
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .px_5()
                .pt_4()
                .pb_2()
                .child(caption(t(lang, "recent")))
                .when(count > 0, |el| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(P::dim())
                            .child(count.to_string()),
                    )
                }),
        )
        .map(|el| {
            if recent.is_empty() {
                el.child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .pb_4()
                        .child(icon("inbox", 22., P::dim()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(P::dim())
                                .child(t(lang, "recent.empty")),
                        ),
                )
            } else {
                el.child(v_flex().px_3().pb_2().children(recent.into_iter().map(|e| {
                    let tone = event_color(&e);
                    let is_in = e.payload.event_type == EventType::In;
                    let (status_icon, status_color) = match e.status {
                        EventStatus::Queued => ("clock-3", P::dim()),
                        EventStatus::Applied => ("check", P::accent()),
                        EventStatus::Duplicate => ("check", P::muted()),
                        EventStatus::Rejected => ("x", P::danger()),
                    };
                    h_flex()
                        .gap_3()
                        .px_2()
                        .py_2()
                        .items_center()
                        .rounded(px(12.))
                        .child(avatar(&e.name, 36., tone))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(e.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if e.status == EventStatus::Rejected {
                                            P::danger()
                                        } else {
                                            tone
                                        })
                                        .child(if e.status == EventStatus::Rejected {
                                            t(lang, "status.rejected")
                                        } else {
                                            t(lang, if is_in { "mode.in" } else { "mode.out" })
                                        }),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_features(tabular())
                                        .text_color(P::muted())
                                        .child(local_time(&e.payload.ts)),
                                )
                                .child(icon(status_icon, 14., status_color)),
                        )
                })))
            }
        })
}

fn local_time(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}

impl Render for KioskApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title: SharedString = t(self.lang(), "title").into();
        window.set_window_title(&title);
        let body = if self.screen == Screen::Setup {
            self.render_setup(cx)
        } else {
            self.render_kiosk(cx)
        };
        div()
            .size_full()
            .bg(linear_gradient(
                180.,
                linear_color_stop(crate::style::hex(0x0B0E11), 0.),
                linear_color_stop(P::bg(), 1.),
            ))
            .text_color(P::text())
            .key_context("Kiosk")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &OpenSetup, _, cx| {
                this.screen = Screen::Setup;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ToggleFullscreen, window, _| window.toggle_fullscreen()))
            .child(body)
    }
}
