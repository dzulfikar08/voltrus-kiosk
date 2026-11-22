//! The window: a setup screen (server, registration keys) and the kiosk
//! screen (camera, who was recognised, recent check-ins, sync status).

use crate::client::Client;
use crate::i18n::t;
use crate::state::{Mode, Seen, Shared};
use crate::store::{Config, EventStatus, EventType, Identity, Lang};
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, RenderImage,
    SharedString, Styled as _, StyledImage as _, Window, actions, div, img, px,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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

    fn render_setup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let theme = cx.theme().clone();
        let uid = self.identity.device_uid.clone();
        let key = self.identity.public_key_b64();
        let field = |label: &'static str, body: AnyElement| {
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(body)
        };
        let copyable = |id: &'static str, label: &'static str, value: String| {
            let copy = value.clone();
            field(
                label,
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(theme.secondary)
                            .font_family("monospace")
                            .text_sm()
                            .child(value),
                    )
                    .child(Button::new(id).label(t(lang, "setup.copy")).on_click(
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        },
                    ))
                    .into_any_element(),
            )
        };
        let lang_button = |id: &'static str, label: &'static str, value: Lang| {
            let b = Button::new(id)
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| this.set_lang(value, cx)));
            if lang == value { b.primary() } else { b }
        };
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(640.))
                    .gap_5()
                    .p_8()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(lang, "setup.title")),
                    )
                    .child(field(
                        t(lang, "setup.server"),
                        Input::new(&self.server).into_any_element(),
                    ))
                    .child(
                        h_flex()
                            .gap_4()
                            .child(div().w(px(160.)).child(field(
                                t(lang, "setup.camera"),
                                Input::new(&self.camera).into_any_element(),
                            )))
                            .child(field(
                                t(lang, "setup.lang"),
                                h_flex()
                                    .gap_2()
                                    .child(lang_button("lang-id", "Indonesia", Lang::Id))
                                    .child(lang_button("lang-en", "English", Lang::En))
                                    .into_any_element(),
                            )),
                    )
                    .child(div().text_sm().child(t(lang, "setup.register")))
                    .child(copyable("copy-uid", t(lang, "setup.uid"), uid))
                    .child(copyable("copy-key", t(lang, "setup.key"), key))
                    .when_some(self.setup_message.clone(), |el, (ok, message)| {
                        el.child(
                            div()
                                .text_sm()
                                .text_color(if ok { theme.success } else { theme.danger })
                                .child(message),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2()
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
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.connect(window, cx)),
                                    ),
                            )
                            .when(self.shared.gallery.lock().is_some(), |el| {
                                el.child(
                                    Button::new("back")
                                        .large()
                                        .label(t(lang, "setup.back"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.screen = Screen::Kiosk;
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(t(lang, "setup.shortcut")),
                    ),
            )
            .into_any_element()
    }

    fn render_kiosk(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let theme = cx.theme().clone();
        let gallery = self.shared.gallery.lock().clone();
        let (seen, _) = self.shared.seen.lock().clone();
        let mode = *self.shared.mode.lock();
        let sync = self.shared.sync.lock().clone();
        let (queued, recent) = {
            let events = self.shared.events.lock();
            (
                events.queue.len(),
                events.log.iter().rev().take(8).cloned().collect::<Vec<_>>(),
            )
        };
        let now = chrono::Local::now();
        let green: Hsla = theme.success;
        let blue: Hsla = theme.info;

        let header = h_flex()
            .justify_between()
            .items_center()
            .px_6()
            .py_3()
            .child(
                v_flex()
                    .child(
                        div().text_xl().font_weight(FontWeight::SEMIBOLD).child(
                            gallery
                                .as_ref()
                                .and_then(|g| g.company.clone())
                                .unwrap_or_else(|| t(lang, "title").into()),
                        ),
                    )
                    .child(
                        div().text_sm().text_color(theme.muted_foreground).child(
                            gallery
                                .as_ref()
                                .map(|g| {
                                    [g.device.name.clone(), g.device.location.clone()]
                                        .into_iter()
                                        .flatten()
                                        .collect::<Vec<_>>()
                                        .join(" · ")
                                })
                                .unwrap_or_default(),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .items_end()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .font_family("monospace")
                            .child(now.format("%H:%M:%S").to_string()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(now.format("%d-%m-%Y").to_string()),
                    ),
            );

        let camera_error = self.shared.camera_error();
        let camera = div()
            .flex_1()
            .h_full()
            .rounded_xl()
            .overflow_hidden()
            .bg(gpui_kit::black())
            .flex()
            .items_center()
            .justify_center()
            .map(|el| match (&self.preview, &camera_error) {
                (Some(p), _) => el.child(img(p.clone()).size_full().object_fit(ObjectFit::Contain)),
                (None, Some(e)) => el.child(
                    v_flex()
                        .items_center()
                        .gap_2()
                        .child(t(lang, "status.camera"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(e.clone()),
                        ),
                ),
                (None, None) => el,
            });

        let mode_button = |id: &'static str, label: &'static str, value: Mode| {
            let b = Button::new(id)
                .large()
                .flex_1()
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    *this.shared.mode.lock() = value;
                    cx.notify();
                }));
            if mode == value { b.primary() } else { b }
        };

        let (headline, detail, color): (String, String, Hsla) = match &seen {
            Seen::Nobody => (
                t(lang, "seen.nobody").into(),
                String::new(),
                theme.foreground,
            ),
            Seen::Unknown => (
                t(lang, "seen.unknown").into(),
                String::new(),
                theme.foreground,
            ),
            Seen::NotLive => (t(lang, "seen.notlive").into(), String::new(), theme.danger),
            Seen::Again { name } => (
                name.clone(),
                t(lang, "seen.again").into(),
                theme.muted_foreground,
            ),
            Seen::Recorded { event } => {
                let is_in = event.payload.event_type == EventType::In;
                let time = local_time(&event.payload.ts);
                (
                    event.name.clone(),
                    format!(
                        "{} · {} {}",
                        t(lang, if is_in { "recorded.in" } else { "recorded.out" }),
                        t(lang, if is_in { "mode.in" } else { "mode.out" }),
                        time
                    ),
                    if is_in { green } else { blue },
                )
            }
        };

        let result = v_flex()
            .gap_2()
            .p_6()
            .rounded_xl()
            .border_2()
            .border_color(color)
            .min_h(px(170.))
            .justify_center()
            .child(
                div()
                    .text_3xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(headline),
            )
            .when(!detail.is_empty(), |el| {
                el.child(div().text_lg().child(detail))
            });

        let recent_list = v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(t(lang, "recent")),
            )
            .children(recent.into_iter().map(|e| {
                let is_in = e.payload.event_type == EventType::In;
                h_flex()
                    .justify_between()
                    .py_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().text_sm().child(e.name.clone()))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if is_in { green } else { blue })
                                    .child(t(lang, if is_in { "mode.in" } else { "mode.out" })),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_family("monospace")
                                    .child(local_time(&e.payload.ts)),
                            )
                            .child(div().text_xs().text_color(theme.muted_foreground).child(
                                match e.status {
                                    EventStatus::Queued => "…",
                                    EventStatus::Applied | EventStatus::Duplicate => "✓",
                                    EventStatus::Rejected => t(lang, "status.rejected"),
                                },
                            )),
                    )
            }));

        let panel = v_flex()
            .w(px(440.))
            .h_full()
            .gap_4()
            .child(
                h_flex()
                    .gap_2()
                    .child(mode_button("mode-auto", t(lang, "mode.auto"), Mode::Auto))
                    .child(mode_button("mode-in", t(lang, "mode.in"), Mode::In))
                    .child(mode_button("mode-out", t(lang, "mode.out"), Mode::Out)),
            )
            .child(result)
            .child(recent_list);

        let engine_error = self.shared.engine_error.lock().clone();
        let status_line = match &gallery {
            None => t(lang, "status.nogallery").to_string(),
            Some(g) => format!(
                "{} {} · {} {} · {} {}",
                g.employees.len(),
                t(lang, "status.people"),
                g.faces.len(),
                t(lang, "status.faces"),
                queued,
                t(lang, "status.queue")
            ),
        };
        let footer = h_flex()
            .justify_between()
            .px_6()
            .py_2()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().size(px(10.)).rounded_full().bg(if sync.online {
                        green
                    } else {
                        theme.warning
                    }))
                    .child(if sync.online {
                        t(lang, "status.online")
                    } else {
                        t(lang, "status.offline")
                    })
                    // Offline is expected and already said; show only real errors.
                    .when_some(
                        if sync.online {
                            sync.error.clone()
                        } else {
                            None
                        }
                        .or(engine_error),
                        |el, e| el.child(div().text_color(theme.danger).child(e)),
                    ),
            )
            .child(status_line);

        v_flex()
            .size_full()
            .child(header)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_4()
                    .px_6()
                    .child(camera)
                    .child(panel),
            )
            .child(footer)
            .into_any_element()
    }
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
        let theme = cx.theme().clone();
        let body = if self.screen == Screen::Setup {
            self.render_setup(cx)
        } else {
            self.render_kiosk(cx)
        };
        div()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
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
