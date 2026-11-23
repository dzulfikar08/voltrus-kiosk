//! The kiosk's admin side: the PIN pad that guards it, and enrolling an
//! employee's face at the kiosk (pick the person, record their consent, take
//! five samples, send them to Voltrus).

use super::{Gate, KioskApp, Screen, avatar, caption, icon, mark};
use crate::i18n::t;
use crate::state::{Capture, CaptureHint};
use crate::store::{Employee, Gallery};
use crate::style::Palette as P;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, FontWeight, InteractiveElement as _, IntoElement,
    KeyDownEvent, ObjectFit, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Window, div, img, px,
};

/// Samples taken per enrolment (the server keeps up to 10).
const SAMPLES: usize = 5;
/// Prompts for each sample, so the templates cover a few head poses.
const POSES: [&str; SAMPLES] = [
    "enroll.pose.front",
    "enroll.pose.left",
    "enroll.pose.right",
    "enroll.pose.up",
    "enroll.pose.front2",
];
const PIN_MAX: usize = 8;

#[derive(Default, Clone, PartialEq)]
pub(super) enum Step {
    #[default]
    Pick,
    Consent,
    Capture,
    Saving,
    /// Templates the employee has now, or the server's refusal.
    Done(Result<usize, String>),
}

#[derive(Default)]
pub(super) struct Enroll {
    pub step: Step,
    pub employee: Option<Employee>,
}

impl KioskApp {
    fn pin_required(&self) -> bool {
        self.admin_pin.is_none()
            && self
                .shared
                .gallery
                .lock()
                .as_ref()
                .is_some_and(|g| g.device.admin_pin_hash.is_some())
    }

    fn pin_is_set(&self) -> bool {
        self.shared
            .gallery
            .lock()
            .as_ref()
            .is_some_and(|g| g.device.admin_pin_hash.is_some())
    }

    /// Open Setup or enrolment, through the PIN pad when HR set a PIN.
    pub(super) fn open_admin(&mut self, gate: Gate, window: &mut Window, cx: &mut Context<Self>) {
        if self.pin_required() {
            self.pin_entry.clear();
            self.pin_wrong = false;
            self.screen = Screen::Pin(gate);
        } else {
            self.enter(gate, window, cx);
        }
        cx.notify();
    }

    fn enter(&mut self, gate: Gate, window: &mut Window, cx: &mut Context<Self>) {
        match gate {
            Gate::Setup => self.screen = Screen::Setup,
            Gate::Enroll => {
                self.enroll = Enroll::default();
                self.search.update(cx, |s, cx| s.set_value("", window, cx));
                self.screen = Screen::Enroll;
            }
        }
    }

    /// Back to the kiosk screen: forget the PIN, stop any capture.
    pub(super) fn go_kiosk(&mut self, cx: &mut Context<Self>) {
        *self.shared.capture.lock() = None;
        self.admin_pin = None;
        self.enroll = Enroll::default();
        self.screen = Screen::Kiosk;
        cx.notify();
    }

    fn pin_push(&mut self, digit: char, cx: &mut Context<Self>) {
        if self.pin_entry.len() < PIN_MAX {
            self.pin_entry.push(digit);
            self.pin_wrong = false;
            cx.notify();
        }
    }

    fn pin_submit(&mut self, gate: Gate, window: &mut Window, cx: &mut Context<Self>) {
        let ok = self.pin_entry.len() >= 4
            && self
                .shared
                .gallery
                .lock()
                .as_ref()
                .is_some_and(|g| g.pin_matches(&self.pin_entry));
        if ok {
            self.admin_pin = Some(std::mem::take(&mut self.pin_entry));
            self.enter(gate, window, cx);
        } else {
            self.pin_entry.clear();
            self.pin_wrong = true;
        }
        cx.notify();
    }

    /// Digits, Backspace, Enter and Escape on the PIN pad.
    pub(super) fn pin_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Screen::Pin(gate) = self.screen else {
            return;
        };
        let key = event.keystroke.key.as_str();
        match key {
            "backspace" => {
                self.pin_entry.pop();
                cx.notify();
            }
            "enter" => self.pin_submit(gate, window, cx),
            "escape" => self.go_kiosk(cx),
            k if k.len() == 1 && k.chars().all(|c| c.is_ascii_digit()) => {
                self.pin_push(k.chars().next().unwrap_or('0'), cx)
            }
            _ => {}
        }
    }

    pub(super) fn render_pin(&mut self, gate: Gate, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let shown = self.pin_entry.len().max(4);
        let dots = h_flex()
            .gap_3()
            .justify_center()
            .children((0..shown).map(|i| {
                let filled = i < self.pin_entry.len();
                div()
                    .size(px(14.))
                    .rounded_full()
                    .when(filled, |el| el.bg(P::text()))
                    .when(!filled, |el| el.border_1().border_color(P::line_strong()))
            }));
        let key = |id: &'static str, content: AnyElement, primary: bool| {
            div()
                .id(id)
                .size(px(76.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_size(px(26.))
                .font_weight(FontWeight::MEDIUM)
                .map(|el| {
                    if primary {
                        el.bg(P::accent()).text_color(P::accent_fg())
                    } else {
                        el.bg(P::raised())
                            .hover(|s| s.bg(crate::style::hex(0x1E242A)))
                    }
                })
                .child(content)
        };
        let digit = |d: char, cx: &mut Context<Self>| {
            let id: &'static str = match d {
                '0' => "pin-0",
                '1' => "pin-1",
                '2' => "pin-2",
                '3' => "pin-3",
                '4' => "pin-4",
                '5' => "pin-5",
                '6' => "pin-6",
                '7' => "pin-7",
                '8' => "pin-8",
                _ => "pin-9",
            };
            key(id, d.to_string().into_any_element(), false)
                .on_click(cx.listener(move |this, _, _, cx| this.pin_push(d, cx)))
        };
        let row = |keys: Vec<gpui_kit::Stateful<gpui_kit::Div>>| h_flex().gap_4().children(keys);
        let pad = v_flex()
            .gap_4()
            .child(row(vec![digit('1', cx), digit('2', cx), digit('3', cx)]))
            .child(row(vec![digit('4', cx), digit('5', cx), digit('6', cx)]))
            .child(row(vec![digit('7', cx), digit('8', cx), digit('9', cx)]))
            .child(row(vec![
                key(
                    "pin-back",
                    icon("delete", 24., P::text()).into_any_element(),
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.pin_entry.pop();
                    cx.notify();
                })),
                digit('0', cx),
                key(
                    "pin-ok",
                    icon("arrow-right", 26., P::accent_fg()).into_any_element(),
                    true,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.pin_submit(gate, window, cx)),
                ),
            ]));
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .items_center()
                    .gap_6()
                    .child(
                        div()
                            .size(px(56.))
                            .rounded(px(16.))
                            .bg(P::raised())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("lock-keyhole", 26., P::text())),
                    )
                    .child(
                        v_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t(lang, "pin.title")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if self.pin_wrong {
                                        P::danger()
                                    } else {
                                        P::muted()
                                    })
                                    .child(if self.pin_wrong {
                                        t(lang, "pin.wrong")
                                    } else {
                                        t(lang, "pin.hint")
                                    }),
                            ),
                    )
                    .child(dots)
                    .child(pad)
                    .child(
                        Button::new("pin-cancel")
                            .ghost()
                            .label(t(lang, "pin.cancel"))
                            .on_click(cx.listener(|this, _, _, cx| this.go_kiosk(cx))),
                    ),
            )
            .into_any_element()
    }

    /// "Enrol a face" in Setup: needs a PIN set in HR → Kiosks.
    pub(super) fn enroll_button(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self.lang();
        let ready = self.pin_is_set();
        Button::new("open-enroll")
            .large()
            .label(t(lang, "enroll.open"))
            .disabled(!ready)
            .when(!ready, |b| b.tooltip(t(lang, "enroll.needpin")))
            .on_click(cx.listener(|this, _, window, cx| this.open_admin(Gate::Enroll, window, cx)))
    }

    /// Send the samples once the capture has all of them.
    pub(super) fn poll_capture(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::Enroll || self.enroll.step != Step::Capture {
            return;
        }
        let samples = {
            let mut slot = self.shared.capture.lock();
            match slot.as_ref() {
                Some(c) if c.done() => slot.take().map(|c| c.samples),
                _ => None,
            }
        };
        let (Some(samples), Some(employee), Some(pin)) = (
            samples,
            self.enroll.employee.clone(),
            self.admin_pin.clone(),
        ) else {
            return;
        };
        self.enroll.step = Step::Saving;
        let (shared, client) = (self.shared.clone(), self.client.clone());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let templates =
                        client
                            .enroll(&pin, &employee.id, &samples)
                            .map_err(|e| match e {
                                crate::client::CallError::Server(_, message) => message,
                                other => other.to_string(),
                            })?;
                    // Recognise them at once rather than at the next pull.
                    let _ = crate::sync::pull_once(&shared, &client);
                    Ok::<usize, String>(templates)
                })
                .await;
            this.update(cx, |this, cx| {
                if this.enroll.step == Step::Saving {
                    this.enroll.step = Step::Done(result);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_capture(&mut self, cx: &mut Context<Self>) {
        *self.shared.capture.lock() = Some(Capture::new(SAMPLES));
        self.enroll.step = Step::Capture;
        cx.notify();
    }

    fn cancel_enroll(&mut self, cx: &mut Context<Self>) {
        *self.shared.capture.lock() = None;
        self.enroll.step = Step::Pick;
        self.enroll.employee = None;
        cx.notify();
    }

    pub(super) fn render_enroll(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let lang = self.lang();
        let gallery = self.shared.gallery.lock().clone();
        let header = h_flex()
            .justify_between()
            .items_center()
            .child(
                h_flex().gap_4().items_center().child(mark(40.)).child(
                    v_flex()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t(lang, "enroll.title")),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(P::muted())
                                .child(t(lang, "enroll.subtitle")),
                        ),
                ),
            )
            .child(
                Button::new("enroll-close")
                    .label(t(lang, "enroll.close"))
                    .on_click(cx.listener(|this, _, _, cx| this.go_kiosk(cx))),
            );
        let body = match self.enroll.step.clone() {
            Step::Pick => self.render_pick(gallery.as_ref(), cx),
            Step::Consent => self.render_consent(cx),
            Step::Capture => self.render_capture(cx),
            Step::Saving => centered(
                v_flex()
                    .items_center()
                    .gap_3()
                    .child(icon("cloud-upload", 40., P::accent()))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t(lang, "enroll.saving")),
                    ),
            ),
            Step::Done(result) => self.render_done(result, cx),
        };
        v_flex()
            .size_full()
            .px_8()
            .pt_6()
            .pb_6()
            .gap_6()
            .child(header)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn render_pick(&mut self, gallery: Option<&Gallery>, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let query = self.search.read(cx).value().to_lowercase();
        let employees: Vec<(Employee, usize)> = gallery
            .map(|g| {
                g.employees
                    .iter()
                    .filter(|e| {
                        query.is_empty()
                            || e.name.to_lowercase().contains(&query)
                            || e.id.to_lowercase().contains(&query)
                    })
                    .map(|e| (e.clone(), g.templates_of(&e.id)))
                    .collect()
            })
            .unwrap_or_default();
        let list = v_flex()
            .id("enroll-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_1()
            .when(employees.is_empty(), |el| {
                el.child(
                    div()
                        .py_8()
                        .text_sm()
                        .text_color(P::dim())
                        .flex()
                        .justify_center()
                        .child(t(lang, "enroll.none")),
                )
            })
            .children(
                employees
                    .into_iter()
                    .enumerate()
                    .map(|(i, (employee, templates))| {
                        let picked = employee.clone();
                        h_flex()
                            .id(("employee", i))
                            .gap_3()
                            .px_3()
                            .py_2p5()
                            .items_center()
                            .rounded(px(12.))
                            .cursor_pointer()
                            .hover(|s| s.bg(P::raised()))
                            .child(avatar(
                                &employee.name,
                                40.,
                                if templates > 0 {
                                    P::accent()
                                } else {
                                    P::muted()
                                },
                            ))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .child(employee.name.clone()),
                                    )
                                    .child(
                                        div().text_xs().text_color(P::muted()).truncate().child(
                                            [Some(employee.id.clone()), employee.position.clone()]
                                                .into_iter()
                                                .flatten()
                                                .collect::<Vec<_>>()
                                                .join(" · "),
                                        ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_full()
                                    .text_xs()
                                    .map(|el| {
                                        if templates > 0 {
                                            el.bg(P::accent().alpha(0.12))
                                                .text_color(P::accent())
                                                .child(icon("check", 12., P::accent()))
                                                .child(t(lang, "enroll.enrolled"))
                                        } else {
                                            el.bg(P::raised())
                                                .text_color(P::muted())
                                                .child(t(lang, "enroll.notenrolled"))
                                        }
                                    }),
                            )
                            .child(icon("chevron-right", 16., P::dim()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.enroll.employee = Some(picked.clone());
                                this.enroll.step = Step::Consent;
                                cx.notify();
                            }))
                    }),
            );
        div()
            .size_full()
            .flex()
            .justify_center()
            .child(
                v_flex()
                    .w(px(720.))
                    .max_w_full()
                    .h_full()
                    .gap_4()
                    .p_5()
                    .rounded(px(20.))
                    .bg(P::surface())
                    .border_1()
                    .border_color(P::line())
                    .child(caption(t(lang, "enroll.pick")))
                    .child(
                        Input::new(&self.search)
                            .large()
                            .prefix(icon("search", 16., P::muted())),
                    )
                    .child(list),
            )
            .into_any_element()
    }

    fn render_consent(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let Some(employee) = self.enroll.employee.clone() else {
            return div().into_any_element();
        };
        centered(
            v_flex()
                .w(px(620.))
                .max_w_full()
                .gap_6()
                .p_8()
                .rounded(px(24.))
                .bg(P::surface())
                .border_1()
                .border_color(P::line_strong())
                .child(
                    h_flex()
                        .gap_4()
                        .items_center()
                        .child(avatar(&employee.name, 64., P::accent()))
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_2xl()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(employee.name.clone()),
                                )
                                .when_some(employee.position.clone(), |el, p| {
                                    el.child(div().text_sm().text_color(P::muted()).child(p))
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .p_4()
                        .rounded(px(14.))
                        .bg(P::bg())
                        .border_1()
                        .border_color(P::line())
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(icon("shield-check", 16., P::accent()))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(t(lang, "enroll.consent.title")),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(P::muted())
                                .child(t(lang, "enroll.consent.body")),
                        ),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .child(
                            Button::new("consent-yes")
                                .primary()
                                .large()
                                .label(t(lang, "enroll.consent.yes"))
                                .on_click(cx.listener(|this, _, _, cx| this.start_capture(cx))),
                        )
                        .child(
                            Button::new("consent-no")
                                .large()
                                .label(t(lang, "enroll.back"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_enroll(cx))),
                        ),
                ),
        )
    }

    fn render_capture(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let capture = self.shared.capture.lock().clone();
        let taken = capture.as_ref().map_or(0, |c| c.samples.len());
        let hint = capture.as_ref().map_or(CaptureHint::NoFace, |c| c.hint);
        let pose = t(lang, POSES[taken.min(SAMPLES - 1)]);
        let (hint_text, hint_color) = match hint {
            CaptureHint::NoFace => (t(lang, "enroll.hint.noface"), P::muted()),
            CaptureHint::TooMany => (t(lang, "enroll.hint.many"), P::warn()),
            CaptureHint::TooSmall => (t(lang, "enroll.hint.small"), P::warn()),
            CaptureHint::NotLive => (t(lang, "enroll.hint.notlive"), P::danger()),
            CaptureHint::Hold => (t(lang, "enroll.hint.hold"), P::muted()),
            CaptureHint::Taken => (t(lang, "enroll.hint.taken"), P::accent()),
        };
        let name = self
            .enroll
            .employee
            .as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_default();
        let stage = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(px(24.))
            .overflow_hidden()
            .bg(gpui_kit::black())
            .border_2()
            .border_color(P::accent().alpha(0.5))
            .when_some(self.preview.clone(), |el, p| {
                el.child(img(p).size_full().object_fit(ObjectFit::Cover))
            });
        let progress = h_flex().gap_2().children((0..SAMPLES).map(|i| {
            div().flex_1().h(px(6.)).rounded_full().bg(if i < taken {
                P::accent()
            } else {
                P::raised()
            })
        }));
        h_flex()
            .size_full()
            .gap_5()
            .child(stage)
            .child(
                v_flex()
                    .w(px(400.))
                    .flex_none()
                    .gap_5()
                    .p_6()
                    .rounded(px(20.))
                    .bg(P::surface())
                    .border_1()
                    .border_color(P::line())
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(avatar(&name, 44., P::accent()))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(name.clone()),
                            ),
                    )
                    .child(progress)
                    .child(
                        div()
                            .text_xs()
                            .text_color(P::muted())
                            .child(format!("{taken} / {SAMPLES}")),
                    )
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(px(34.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(pose),
                    )
                    .child(div().text_base().text_color(hint_color).child(hint_text))
                    .child(div().flex_1())
                    .child(
                        Button::new("capture-cancel")
                            .large()
                            .label(t(lang, "enroll.cancel"))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_enroll(cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_done(&mut self, result: Result<usize, String>, cx: &mut Context<Self>) -> AnyElement {
        let lang = self.lang();
        let name = self
            .enroll
            .employee
            .as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_default();
        let (icon_name, tone, title, detail) = match &result {
            Ok(n) => (
                "circle-check",
                P::accent(),
                format!("{} {name}", t(lang, "enroll.done")),
                format!("{n} {}", t(lang, "enroll.done.detail")),
            ),
            Err(e) => (
                "circle-alert",
                P::danger(),
                t(lang, "enroll.failed").to_string(),
                e.clone(),
            ),
        };
        centered(
            v_flex()
                .w(px(560.))
                .max_w_full()
                .items_center()
                .gap_5()
                .p_8()
                .rounded(px(24.))
                .bg(P::surface())
                .border_1()
                .border_color(tone.alpha(0.35))
                .child(
                    div()
                        .size(px(72.))
                        .rounded_full()
                        .bg(tone.alpha(0.12))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(icon_name, 36., tone)),
                )
                .child(
                    v_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(div().text_sm().text_color(P::muted()).child(detail)),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .when(result.is_err(), |el| {
                            el.child(
                                Button::new("enroll-retry")
                                    .primary()
                                    .large()
                                    .label(t(lang, "enroll.retry"))
                                    .on_click(cx.listener(|this, _, _, cx| this.start_capture(cx))),
                            )
                        })
                        .child(
                            Button::new("enroll-another")
                                .when(result.is_ok(), |b| b.primary())
                                .large()
                                .label(t(lang, "enroll.another"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_enroll(cx))),
                        )
                        .child(
                            Button::new("enroll-finish")
                                .large()
                                .label(t(lang, "enroll.finish"))
                                .on_click(cx.listener(|this, _, _, cx| this.go_kiosk(cx))),
                        ),
                ),
        )
    }
}

fn centered(content: impl IntoElement) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(content)
        .into_any_element()
}
