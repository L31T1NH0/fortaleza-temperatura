use crate::model::*;
use iced::{
    Color, Element, Fill, Font, Pixels, Point, Rectangle, Renderer, Size, Task, Theme, mouse,
    widget::{
        canvas,
        canvas::{Frame, Geometry, Path, Stroke, Text},
        column, container, pick_list, row, space, text,
    },
};
use std::{fmt, sync::Arc};
const BG: Color = Color::from_rgb8(24, 30, 30);
const MUTED: Color = Color::from_rgb8(126, 147, 117);
const FG: Color = Color::from_rgb8(225, 230, 215);
const GRID: Color = Color::from_rgb8(48, 61, 46);
const PALETTE: [[u8; 3]; 8] = [
    [163, 182, 145],
    [215, 180, 139],
    [170, 162, 195],
    [134, 171, 180],
    [199, 143, 133],
    [188, 182, 129],
    [143, 186, 169],
    [195, 157, 173],
];
fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgb8(c[0], c[1], c[2])
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice {
    index: Option<usize>,
    name: String,
}
impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}
struct Shape {
    region: Region,
    rings: Vec<Vec<[f32; 2]>>,
}
struct Panel {
    shapes: Vec<Shape>,
    choices: Vec<Choice>,
    selected: Option<usize>,
    data: Option<Arc<Data>>,
    error: bool,
    narrow: bool,
}
#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Arc<Data>, String>),
    Selected(Option<usize>),
    Picked(Choice),
    Resized(Size),
}
pub fn run() -> iced::Result {
    iced::application(Panel::new, Panel::update, Panel::view)
        .title("Fortaleza · Temperatura")
        .default_font(Font::with_name("Fira Sans"))
        .theme(|_: &Panel| {
            Theme::custom(
                "Fortaleza",
                iced::theme::Palette {
                    background: BG,
                    text: FG,
                    primary: rgb([156, 175, 136]),
                    success: rgb([156, 175, 136]),
                    warning: rgb([211, 181, 141]),
                    danger: rgb([202, 125, 102]),
                },
            )
        })
        .subscription(|_: &Panel| iced::window::resize_events().map(|(_, s)| Message::Resized(s)))
        .run()
}
fn fetch(delay: bool) -> Task<Message> {
    Task::perform(
        async move {
            if delay {
                gloo_timers::future::TimeoutFuture::new(5000).await;
            }
            let response = gloo_net::http::Request::get("/api/collector")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !response.ok() {
                return Err("Não foi possível consultar o coletor".into());
            }
            serde_json::from_str::<Data>(&response.text().await.map_err(|e| e.to_string())?)
                .map(Arc::new)
                .map_err(|e| e.to_string())
        },
        Message::Loaded,
    )
}
impl Panel {
    fn new() -> (Self, Task<Message>) {
        let regions: Vec<Region> =
            serde_json::from_str(include_str!("../../data/map.json")).expect("Mapa inválido");
        let mut choices = vec![Choice {
            index: None,
            name: "Todos os bairros".into(),
        }];
        let shapes = regions
            .into_iter()
            .enumerate()
            .map(|(i, region)| {
                choices.push(Choice {
                    index: Some(i),
                    name: region.name.clone(),
                });
                let rings = rings(&region.path);
                Shape { region, rings }
            })
            .collect();
        (
            Self {
                shapes,
                choices,
                selected: None,
                data: None,
                error: false,
                narrow: false,
            },
            fetch(false),
        )
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(result) => {
                match result {
                    Ok(data) => {
                        self.data = Some(data);
                        self.error = false
                    }
                    Err(_) => self.error = true,
                }
                return fetch(true);
            }
            Message::Selected(i) => self.selected = i,
            Message::Picked(choice) => self.selected = choice.index,
            Message::Resized(s) => self.narrow = s.width < 520.,
        }
        Task::none()
    }
    fn latest(&self) -> Option<&Snapshot> {
        self.data.as_ref()?.latest.as_ref()
    }
    fn reading(&self, index: usize) -> Option<&Reading> {
        let id = &self.shapes.get(index)?.region.id;
        self.latest()?.leituras.iter().find(|r| r.bairro_id == *id)
    }
    fn limits(&self) -> (f64, f64) {
        let values = self
            .latest()
            .map(|s| {
                s.leituras
                    .iter()
                    .map(|r| r.temperatura_c)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        (
            values.iter().copied().reduce(f64::min).unwrap_or(24.),
            values.iter().copied().reduce(f64::max).unwrap_or(34.),
        )
    }
    fn status(&self) -> &str {
        if self.error {
            return "Sem conexão";
        }
        let Some(d) = &self.data else {
            return "Conectando";
        };
        if d.service.state != "active" || d.service.detail != "running" {
            return "Coletor parado";
        }
        if self
            .latest()
            .and_then(|s| {
                Some(
                    time(&d.server_time)?.timestamp_millis()
                        - time(&s.coletado_em)?.timestamp_millis(),
                )
            })
            .is_none_or(|age| age > 75 * 60000)
        {
            "Coleta atrasada"
        } else if !d.issues.is_empty() {
            "Verifique os logs"
        } else {
            "Coletando"
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let last = self
            .latest()
            .map(|s| clock(&s.coletado_em))
            .unwrap_or_else(|| "—".into());
        let status = text(format!("{} · {}", self.status(), last))
            .size(11)
            .color(MUTED);
        let choice = self
            .choices
            .iter()
            .find(|c| c.index == self.selected)
            .cloned();
        let picker = pick_list(self.choices.as_slice(), choice, Message::Picked)
            .text_size(10)
            .width(155);
        let header: Element<'_, Message> = if self.narrow {
            column![
                row![text("Fortaleza").size(25), space::horizontal(), status].align_y(iced::Center),
                row![space::horizontal(), picker]
            ]
            .spacing(12)
            .into()
        } else {
            row![
                text("Fortaleza").size(25),
                space::horizontal(),
                picker,
                status
            ]
            .spacing(22)
            .align_y(iced::Center)
            .into()
        };
        container(
            column![
                header,
                canvas(Scene(self)).width(Fill).height(Fill),
                row![
                    text("Open-Meteo · estimativas").size(9).color(MUTED),
                    space::horizontal(),
                    text("Atualiza automaticamente").size(9).color(MUTED)
                ]
            ]
            .spacing(20),
        )
        .padding(24)
        .width(Fill)
        .height(Fill)
        .into()
    }
}
struct Scene<'a>(&'a Panel);
#[derive(Default)]
struct SceneState {
    hover: Option<usize>,
}
struct Dot<'a> {
    reading: &'a Reading,
    time: &'a str,
    bairro: usize,
    position: Point,
}
struct Layout {
    map: Rectangle,
    chart: Rectangle,
    map_footer: f32,
}
fn layout(size: Size) -> Layout {
    if size.width < 500. {
        let split = size.height * 0.48;
        Layout {
            map: Rectangle {
                x: 0.,
                y: 25.,
                width: size.width,
                height: (split - 75.).max(1.),
            },
            chart: Rectangle {
                x: 0.,
                y: split + 30.,
                width: size.width,
                height: (size.height - split - 65.).max(1.),
            },
            map_footer: split - 42.,
        }
    } else {
        let map_width = size.width * 0.52 - 18.;
        Layout {
            map: Rectangle {
                x: 0.,
                y: 25.,
                width: map_width,
                height: (size.height - 110.).max(1.),
            },
            chart: Rectangle {
                x: map_width + 36.,
                y: 25.,
                width: (size.width - map_width - 36.).max(1.),
                height: (size.height - 60.).max(1.),
            },
            map_footer: size.height - 65.,
        }
    }
}
fn transform(rect: Rectangle) -> (f32, Point) {
    let scale = (rect.width / 960.).min(rect.height / 960.).max(0.001);
    (
        scale,
        Point::new(
            rect.x + (rect.width - 960. * scale) / 2.,
            rect.y + (rect.height - 960. * scale) / 2.,
        ),
    )
}
impl Scene<'_> {
    fn points(&self, rect: Rectangle) -> (Vec<Dot<'_>>, f64, f64, i64) {
        let Some(d) = &self.0.data else {
            return (vec![], 20., 35., 0);
        };
        let rows: Vec<_> = d
            .recent
            .iter()
            .flat_map(|s| s.leituras.iter().map(move |r| (r, s.coletado_em.as_str())))
            .collect();
        let low = rows
            .iter()
            .map(|(r, _)| r.temperatura_c)
            .reduce(f64::min)
            .unwrap_or(21.)
            .floor()
            - 1.;
        let high = rows
            .iter()
            .map(|(r, _)| r.temperatura_c)
            .reduce(f64::max)
            .unwrap_or(34.)
            .ceil()
            + 1.;
        let end = time(&d.server_time)
            .map(|t| t.timestamp_millis())
            .unwrap_or(0);
        let dots = rows
            .into_iter()
            .filter_map(|(reading, t)| {
                Some(Dot {
                    reading,
                    time: t,
                    bairro: self
                        .0
                        .shapes
                        .iter()
                        .position(|s| s.region.id == reading.bairro_id)?,
                    position: Point::new(
                        rect.x
                            + 36.
                            + (time(t)?.timestamp_millis() - end + 86400000) as f32 / 86400000.
                                * (rect.width - 48.),
                        rect.y + rect.height
                            - 24.
                            - ((reading.temperatura_c - low) / (high - low)) as f32
                                * (rect.height - 36.),
                    ),
                })
            })
            .collect();
        (dots, low, high, end)
    }
}
// One Iced Canvas keeps the WebGL draw batches in the same coordinate system.
impl canvas::Program<Message> for Scene<'_> {
    type State = SceneState;
    fn update(
        &self,
        state: &mut SceneState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let l = layout(bounds.size());
        let p = cursor.position_in(bounds);
        if let Some(p) = p {
            if l.map.contains(p)
                && matches!(
                    event,
                    canvas::Event::Mouse(mouse::Event::CursorMoved { .. })
                        | canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                        | canvas::Event::Touch(iced::touch::Event::FingerPressed { .. })
                )
            {
                let (scale, origin) = transform(l.map);
                let q = [(p.x - origin.x) / scale, (p.y - origin.y) / scale];
                let i = self.0.shapes.iter().position(|s| contains(q, &s.rings));
                if i.is_some() && i != self.0.selected {
                    return Some(canvas::Action::publish(Message::Selected(i)));
                }
            }
        }
        let (dots, _, _, _) = self.points(l.chart);
        let hover = p.and_then(|p| {
            dots.iter()
                .enumerate()
                .filter(|(_, d)| d.position.distance(p) < 8.)
                .min_by(|(_, a), (_, b)| {
                    (self.0.selected == Some(b.bairro))
                        .cmp(&(self.0.selected == Some(a.bairro)))
                        .then_with(|| a.position.distance(p).total_cmp(&b.position.distance(p)))
                })
                .map(|(i, _)| i)
        });
        if matches!(
            event,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            if let Some(i) = hover {
                return Some(canvas::Action::publish(Message::Selected(Some(
                    dots[i].bairro,
                ))));
            }
        }
        if hover != state.hover {
            state.hover = hover;
            return Some(canvas::Action::request_redraw());
        }
        None
    }
    fn draw(
        &self,
        state: &SceneState,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        if bounds.width < 80. || bounds.height < 80. {
            return vec![];
        }
        let mut frame = Frame::new(renderer, bounds.size());
        let l = layout(bounds.size());
        label(&mut frame, "Agora".into(), Point::ORIGIN, 12., MUTED);
        let last = self
            .0
            .latest()
            .map(|s| clock(&s.coletado_em))
            .unwrap_or_else(|| "—".into());
        label_right(&mut frame, last, Point::new(l.map.width, 0.), 10., MUTED);
        label(
            &mut frame,
            "Últimas 24h".into(),
            Point::new(l.chart.x, l.chart.y - 25.),
            12.,
            MUTED,
        );
        let (scale, origin) = transform(l.map);
        let (low, high) = self.0.limits();
        for (i, shape) in self.0.shapes.iter().enumerate() {
            let path = Path::new(|builder| {
                for ring in &shape.rings {
                    if let Some(first) = ring.first() {
                        builder.move_to(Point::new(
                            first[0] * scale + origin.x,
                            first[1] * scale + origin.y,
                        ));
                        for p in ring.iter().skip(1) {
                            builder.line_to(Point::new(
                                p[0] * scale + origin.x,
                                p[1] * scale + origin.y,
                            ));
                        }
                        builder.close();
                    }
                }
            });
            let c = self
                .0
                .reading(i)
                .map(|r| rgb(thermal(r.temperatura_c, low, high)))
                .unwrap_or(rgb([48, 60, 58]));
            frame.fill(
                &path,
                canvas::Fill {
                    style: canvas::Style::Solid(c),
                    rule: canvas::fill::Rule::EvenOdd,
                },
            );
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(if self.0.selected == Some(i) {
                        rgb([240, 228, 200])
                    } else {
                        BG
                    })
                    .with_width(if self.0.selected == Some(i) { 2. } else { 0.7 }),
            );
        }
        let name = self
            .0
            .selected
            .map(|i| self.0.shapes[i].region.name.clone())
            .unwrap_or_else(|| "Explore os bairros".into());
        label(&mut frame, name, Point::new(0., l.map_footer), 11., MUTED);
        let value = self
            .0
            .selected
            .and_then(|i| self.0.reading(i))
            .map(|r| format!("{}°", number(r.temperatura_c)))
            .unwrap_or_else(|| "—".into());
        label(
            &mut frame,
            value,
            Point::new(0., l.map_footer + 20.),
            27.,
            FG,
        );
        let left = (l.map.width - 115.).max(0.);
        for i in 0..100 {
            frame.fill(
                &Path::rectangle(
                    Point::new(left + i as f32 / 100. * 115., l.map_footer + 26.),
                    Size::new(2., 4.),
                ),
                rgb(thermal(i as f64 / 100., 0., 1.)),
            );
        }
        label(
            &mut frame,
            format!("{}°", number(low)),
            Point::new(left, l.map_footer + 37.),
            9.,
            MUTED,
        );
        label_right(
            &mut frame,
            format!("{}°", number(high)),
            Point::new(l.map.width, l.map_footer + 37.),
            9.,
            MUTED,
        );
        let (dots, min, max, end) = self.points(l.chart);
        for value in [min, (min + max) / 2., max] {
            let y = l.chart.y + l.chart.height
                - 24.
                - ((value - min) / (max - min)) as f32 * (l.chart.height - 36.);
            for x in (36..(l.chart.width as usize).saturating_sub(12)).step_by(8) {
                frame.stroke(
                    &Path::line(
                        Point::new(l.chart.x + x as f32, y),
                        Point::new(l.chart.x + x as f32 + 2., y),
                    ),
                    Stroke::default().with_color(GRID).with_width(1.),
                );
            }
            label_right(
                &mut frame,
                format!("{}°", number(value)),
                Point::new(l.chart.x + 26., y - 5.),
                10.,
                MUTED,
            );
        }
        for i in 0..5 {
            let value = if self.0.data.is_some() {
                stamp(end - 86400000 + 86400000 * i / 4)
            } else {
                "—".into()
            };
            let align = if i == 0 {
                iced::widget::text::Alignment::Left
            } else if i == 4 {
                iced::widget::text::Alignment::Right
            } else {
                iced::widget::text::Alignment::Center
            };
            draw_text(
                &mut frame,
                Text {
                    content: value,
                    position: Point::new(
                        l.chart.x + 36. + i as f32 / 4. * (l.chart.width - 48.),
                        l.chart.y + l.chart.height - 10.,
                    ),
                    size: Pixels(10.),
                    color: MUTED,
                    align_x: align,
                    ..Text::default()
                },
            );
        }
        let mut ordered: Vec<_> = dots.iter().collect();
        ordered.sort_by_key(|d| self.0.selected == Some(d.bairro));
        for dot in ordered {
            let selected = self.0.selected == Some(dot.bairro);
            let mut c = rgb(PALETTE[dot.bairro % PALETTE.len()]);
            c.a = if self.0.selected.is_none() || selected {
                0.8
            } else {
                0.15
            };
            frame.fill(
                &Path::circle(dot.position, if selected { 4. } else { 3. }),
                c,
            );
        }
        if dots.is_empty() {
            label(
                &mut frame,
                "Aguardando registros".into(),
                Point::new(l.chart.x + 45., l.chart.y + l.chart.height / 2.),
                12.,
                MUTED,
            );
        }
        if let Some(dot) = state.hover.and_then(|i| dots.get(i)) {
            let origin = Point::new(l.chart.x + 45., l.chart.y + 10.);
            frame.fill(
                &Path::rectangle(origin, Size::new(190., 80.)),
                rgb([43, 56, 44]),
            );
            label(
                &mut frame,
                dot.reading.bairro.clone(),
                Point::new(origin.x + 12., origin.y + 10.),
                11.,
                FG,
            );
            label(
                &mut frame,
                format!("{}°C", number(dot.reading.temperatura_c)),
                Point::new(origin.x + 12., origin.y + 29.),
                23.,
                rgb([231, 221, 189]),
            );
            label(
                &mut frame,
                clock(dot.time),
                Point::new(origin.x + 12., origin.y + 60.),
                10.,
                MUTED,
            );
        }
        let caption_y = l.chart.y + l.chart.height + 11.;
        label(
            &mut frame,
            "Um ponto por bairro e coleta".into(),
            Point::new(l.chart.x, caption_y),
            9.,
            MUTED,
        );
        label_right(
            &mut frame,
            format!(
                "{} coletas",
                self.0.data.as_ref().map_or(0, |d| d.recent.len())
            ),
            Point::new(l.chart.x + l.chart.width, caption_y),
            9.,
            MUTED,
        );
        vec![frame.into_geometry()]
    }
    fn mouse_interaction(
        &self,
        _: &SceneState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
fn draw_text(frame: &mut Frame, mut value: Text) {
    value.font = Font::with_name("Fira Sans");
    value.draw_with(|path, color| frame.fill(&path, color));
}
fn label(frame: &mut Frame, content: String, position: Point, size: f32, color: Color) {
    draw_text(
        frame,
        Text {
            content,
            position,
            size: Pixels(size),
            color,
            ..Text::default()
        },
    );
}
fn label_right(frame: &mut Frame, content: String, position: Point, size: f32, color: Color) {
    draw_text(
        frame,
        Text {
            content,
            position,
            size: Pixels(size),
            color,
            align_x: iced::widget::text::Alignment::Right,
            ..Text::default()
        },
    );
}
