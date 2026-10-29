mod currency;
mod document;
mod formula;
mod ods_rules;
use document::Document;
use eframe::egui::{self, Align2, Color32, FontId, Key, RichText, Stroke, Vec2};
use std::path::PathBuf;

const INK: Color32 = Color32::from_rgb(30, 48, 74);
const BLUE: Color32 = Color32::from_rgb(40, 94, 196);
const MUTED: Color32 = Color32::from_rgb(108, 124, 145);
const LINE: Color32 = Color32::from_rgb(222, 230, 240);
const PALE: Color32 = Color32::from_rgb(241, 246, 253);

#[derive(Clone, Copy)]
enum Pending {
    New,
    Example,
    Open,
    Close,
}
struct EasySheet {
    doc: Document,
    path: Option<PathBuf>,
    selected: (u32, u32),
    anchor: (u32, u32),
    edit: String,
    editing: bool,
    focus_edit: bool,
    guide: bool,
    step: usize,
    status: String,
    error: Option<String>,
    pending: Option<Pending>,
    confirm: bool,
    conditional: bool,
    threshold: f64,
    operator: String,
    rows: usize,
    cols: usize,
    jump: String,
    rename: String,
    rename_open: bool,
    allow_close: bool,
    function_search: String,
    currency_open: bool,
    currency_settings: currency::CurrencySettings,
}
impl EasySheet {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.override_text_color = Some(INK);
        style.visuals.panel_fill = Color32::WHITE;
        style.visuals.window_fill = Color32::WHITE;
        style.visuals.selection.bg_fill = BLUE;
        style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        style.visuals.widgets.inactive.bg_fill = PALE;
        style.visuals.widgets.inactive.weak_bg_fill = PALE;
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(225, 236, 252);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(210, 226, 250);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(22.0, egui::FontFamily::Name("bold".into())),
        );
        cc.egui_ctx.set_style(style);
        let mut app = Self {
            doc: Document::demo(),
            path: None,
            selected: (3, 1),
            anchor: (3, 1),
            edit: String::new(),
            editing: false,
            focus_edit: false,
            guide: true,
            step: 0,
            status: "Example workbook · explore it, or start a blank sheet".into(),
            error: None,
            pending: None,
            confirm: false,
            conditional: false,
            threshold: 0.0,
            operator: "<".into(),
            rows: 200,
            cols: 26,
            jump: String::new(),
            rename: String::new(),
            rename_open: false,
            allow_close: false,
            function_search: String::new(),
            currency_open: false,
            currency_settings: currency::CurrencySettings::default(),
        };
        if let Some(p) = std::env::args().nth(1) {
            app.load(PathBuf::from(p));
        }
        app.sync_edit();
        app
    }
    fn sync_edit(&mut self) {
        self.edit = self.doc.raw(self.selected.0, self.selected.1);
        self.jump = formula::address(self.selected.0, self.selected.1);
    }
    fn commit(&mut self) {
        if self.editing {
            self.doc.set(self.selected.0, self.selected.1, &self.edit);
            self.editing = false;
        }
    }
    fn select(&mut self, p: (u32, u32), extend: bool) {
        self.commit();
        self.selected = p;
        if !extend {
            self.anchor = p;
        }
        self.sync_edit();
    }
    fn bounds(&self) -> ((u32, u32), (u32, u32)) {
        (
            (
                self.selected.0.min(self.anchor.0),
                self.selected.1.min(self.anchor.1),
            ),
            (
                self.selected.0.max(self.anchor.0),
                self.selected.1.max(self.anchor.1),
            ),
        )
    }
    fn range_label(&self) -> String {
        let (a, b) = self.bounds();
        if a == b {
            formula::address(a.0, a.1)
        } else {
            format!(
                "{}:{}",
                formula::address(a.0, a.1),
                formula::address(b.0, b.1)
            )
        }
    }
    fn load(&mut self, p: PathBuf) {
        match Document::open(&p) {
            Ok(d) => {
                self.doc = d;
                self.path = Some(p);
                self.selected = (0, 0);
                self.anchor = (0, 0);
                self.editing = false;
                self.rows = 200;
                self.cols = 26;
                self.sync_edit();
                self.status = "Opened OpenDocument workbook".into();
            }
            Err(e) => self.error = Some(format!("Could not open this workbook: {e}")),
        }
    }
    fn save(&mut self, save_as: bool) -> bool {
        self.commit();
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("OpenDocument spreadsheet", &["ods"])
                .set_file_name(
                    self.path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|p| p.to_str())
                        .unwrap_or("Untitled.ods"),
                )
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(mut p) = path else { return false };
        if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("ods")) {
            p.set_extension("ods");
        }
        match self.doc.save(&p) {
            Ok(()) => {
                self.status = format!(
                    "Saved {}",
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
                self.path = Some(p);
                true
            }
            Err(e) => {
                self.error = Some(format!(
                    "Could not save the workbook. Your edits are still here.\n{e}"
                ));
                false
            }
        }
    }
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        self.commit();
        if self.doc.dirty {
            self.pending = Some(action);
            self.confirm = true;
        } else {
            self.execute(action, ctx)
        }
    }
    fn execute(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::New | Pending::Example => {
                self.doc = if matches!(action, Pending::New) {
                    Document::blank()
                } else {
                    Document::demo()
                };
                self.path = None;
                self.selected = (0, 0);
                self.anchor = (0, 0);
                self.rows = 200;
                self.cols = 26;
                self.sync_edit();
                self.status = "Ready when you are".into();
            }
            Pending::Open => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("OpenDocument spreadsheet", &["ods"])
                    .pick_file()
                {
                    self.load(p)
                }
            }
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn apply_style(&mut self, property: &str, value: &str) {
        self.commit();
        self.doc.checkpoint();
        let (a, b) = self.bounds();
        for r in a.0..=b.0 {
            for c in a.1..=b.1 {
                self.doc.style_cell(r, c, |s| {
                    if property == "fo:background-color" {
                        s.cellstyle_mut().set_attr(property, value);
                    } else if property == "fo:text-align" {
                        s.paragraphstyle_mut().set_attr(property, value);
                    } else {
                        s.textstyle_mut().set_attr(property, value);
                    }
                });
            }
        }
    }
    fn clear(&mut self) {
        self.commit();
        self.doc.checkpoint();
        let (a, b) = self.bounds();
        for r in a.0..=b.0 {
            for c in a.1..=b.1 {
                self.doc.write(r, c, "");
            }
        }
        self.doc.recalculate();
        self.sync_edit();
    }
    fn paste(&mut self, text: &str) {
        self.commit();
        self.doc.checkpoint();
        for (r, line) in text
            .trim_end_matches(['\r', '\n'])
            .split('\n')
            .take(10000)
            .enumerate()
        {
            for (c, v) in line
                .trim_end_matches('\r')
                .split('\t')
                .take(1000)
                .enumerate()
            {
                self.doc
                    .write(self.selected.0 + r as u32, self.selected.1 + c as u32, v);
            }
        }
        self.doc.recalculate();
        self.sync_edit();
        self.status = "Pasted cells".into();
    }
    fn copy(&mut self, ctx: &egui::Context) {
        self.commit();
        let (a, b) = self.bounds();
        let mut lines = vec![];
        for r in a.0..=b.0 {
            let mut cells = vec![];
            for c in a.1..=b.1 {
                cells.push(self.doc.raw(r, c));
            }
            lines.push(cells.join("\t"));
        }
        ctx.copy_text(lines.join("\n"));
        self.status = format!("Copied {}", self.range_label());
    }
    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.save(ctx.input(|i| i.modifiers.shift));
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::O)) {
            self.request(Pending::Open, ctx);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::N)) {
            self.request(Pending::New, ctx);
        }
        if self.confirm
            || self.conditional
            || self.currency_open
            || self.rename_open
            || self.error.is_some()
        {
            return;
        }
        if self.editing
            && ctx.wants_keyboard_input()
            && !self.completion_prefix().is_empty()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Tab))
        {
            let prefix = self.completion_prefix();
            if let Some((name, _, _)) = formula::FUNCTIONS
                .iter()
                .find(|(n, _, _)| n.starts_with(&prefix))
            {
                self.complete_function(name);
                self.focus_edit = true;
            }
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Z)) {
                if ctx.input(|i| i.modifiers.shift) {
                    self.doc.redo()
                } else {
                    self.doc.undo()
                }
                self.sync_edit();
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::B)) {
                self.apply_style("fo:font-weight", "bold");
            }
            let mut typed = String::new();
            for e in ctx.input(|i| i.events.clone()) {
                match e {
                    egui::Event::Copy => self.copy(ctx),
                    egui::Event::Cut => {
                        self.copy(ctx);
                        self.clear();
                    }
                    egui::Event::Paste(t) => self.paste(&t),
                    egui::Event::Text(t) if !ctx.input(|i| i.modifiers.command) => {
                        typed.push_str(&t);
                    }
                    _ => {}
                }
            }
            if !typed.is_empty() {
                self.edit = typed;
                self.editing = true;
                self.focus_edit = true;
            }
            let shift = ctx.input(|i| i.modifiers.shift);
            let (r, c) = self.selected;
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                self.select((r + 1, c), shift)
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                self.select((r.saturating_sub(1), c), shift)
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Tab)) {
                self.select((r, c + 1), shift)
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
                self.select((r, c.saturating_sub(1)), shift)
            }
            if ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::F2)) {
                self.editing = true;
                self.focus_edit = true;
            }
            if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                self.clear();
            }
        }
    }
    fn completion_prefix(&self) -> String {
        if !self.edit.starts_with('=') {
            return String::new();
        }
        self.edit
            .rsplit(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("")
            .to_uppercase()
    }
    fn complete_function(&mut self, name: &str) {
        let prefix = self.completion_prefix();
        self.edit.truncate(self.edit.len() - prefix.len());
        self.edit.push_str(name);
        self.edit.push('(');
        self.editing = true;
    }
    fn quick_sum(&mut self) {
        self.commit();
        let (r, c) = self.selected;
        let (a, b) = self.bounds();
        if a != b {
            self.edit = format!(
                "=SUM({}:{})",
                formula::address(a.0, a.1),
                formula::address(b.0, b.1)
            );
            self.selected = (b.0 + 1, b.1);
            self.anchor = self.selected;
        } else if r > 0 {
            let mut start = r - 1;
            while start > 0 && !self.doc.raw(start - 1, c).is_empty() {
                start -= 1;
            }
            self.edit = format!(
                "=SUM({}:{})",
                formula::address(start, c),
                formula::address(r - 1, c)
            );
        } else {
            self.status = "Select numbers, or choose a cell below the numbers to add.".into();
            return;
        }
        self.editing = true;
        self.focus_edit = true;
        self.jump = formula::address(self.selected.0, self.selected.1);
    }
    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(22, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new()
                        .fill(BLUE)
                        .corner_radius(9)
                        .inner_margin(9)
                        .show(ui, |ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                            ui.painter().rect_stroke(
                                rect,
                                3,
                                Stroke::new(1.5_f32, Color32::WHITE),
                                egui::StrokeKind::Inside,
                            );
                            for offset in [8.0, 16.0] {
                                ui.painter().line_segment(
                                    [
                                        rect.min + Vec2::new(offset, 0.0),
                                        rect.min + Vec2::new(offset, 24.0),
                                    ],
                                    Stroke::new(1.0_f32, Color32::WHITE),
                                );
                                ui.painter().line_segment(
                                    [
                                        rect.min + Vec2::new(0.0, offset),
                                        rect.min + Vec2::new(24.0, offset),
                                    ],
                                    Stroke::new(1.0_f32, Color32::WHITE),
                                );
                            }
                        });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("easysheet").strong().size(22.0));
                        ui.label(
                            RichText::new("A little space to think in cells.")
                                .color(MUTED)
                                .size(11.0),
                        );
                    });
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        let title = self
                            .path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or("Untitled workbook".into());
                        ui.label(
                            RichText::new(format!(
                                "{}{}",
                                title,
                                if self.doc.dirty { " •" } else { "" }
                            ))
                            .strong(),
                        );
                        ui.label(
                            RichText::new("OpenDocument  /  .ods")
                                .color(MUTED)
                                .size(11.0),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Save workbook").color(Color32::WHITE),
                                )
                                .fill(BLUE),
                            )
                            .clicked()
                        {
                            self.save(false);
                        }
                        if ui.button("Open").clicked() {
                            self.request(Pending::Open, ctx)
                        }
                        ui.menu_button("File", |ui| {
                            if ui.button("New workbook       ⌘N").clicked() {
                                ui.close();
                                self.request(Pending::New, ctx)
                            }
                            if ui.button("Open workbook      ⌘O").clicked() {
                                ui.close();
                                self.request(Pending::Open, ctx)
                            }
                            if ui.button("Save as…            ⇧⌘S").clicked() {
                                ui.close();
                                self.save(true);
                            }
                            ui.separator();
                            if ui.button("Explore example").clicked() {
                                ui.close();
                                self.request(Pending::Example, ctx)
                            }
                        });
                    });
                });
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button("Undo")
                        .on_hover_text("Undo · Cmd/Ctrl+Z")
                        .clicked()
                    {
                        self.commit();
                        self.doc.undo();
                        self.sync_edit();
                    }
                    if ui
                        .button("Redo")
                        .on_hover_text("Redo · Shift+Cmd/Ctrl+Z")
                        .clicked()
                    {
                        self.commit();
                        self.doc.redo();
                        self.sync_edit();
                    }
                    ui.separator();
                    let s = self.doc.base_style(self.selected.0, self.selected.1);
                    let bold = s.textstyle().attr("fo:font-weight") == Some("bold");
                    let italic = s.textstyle().attr("fo:font-style") == Some("italic");
                    if ui
                        .selectable_label(bold, RichText::new("B").strong())
                        .on_hover_text("Bold")
                        .clicked()
                    {
                        self.apply_style("fo:font-weight", if bold { "normal" } else { "bold" });
                    }
                    if ui
                        .selectable_label(italic, RichText::new("I").italics())
                        .on_hover_text("Italic")
                        .clicked()
                    {
                        self.apply_style("fo:font-style", if italic { "normal" } else { "italic" });
                    }
                    ui.menu_button("Text size", |ui| {
                        for n in [10, 12, 14, 16, 20, 24] {
                            if ui.button(format!("{n} pt")).clicked() {
                                self.apply_style("fo:font-size", &format!("{n}pt"));
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Fill", |ui| {
                        for (name, color) in [
                            ("Clear", "transparent"),
                            ("Ice blue", "#e4edf9"),
                            ("Mint", "#dff3ea"),
                            ("Butter", "#fff2c9"),
                            ("Rose", "#ffe1de"),
                            ("White", "#ffffff"),
                        ] {
                            if ui.button(name).clicked() {
                                self.apply_style("fo:background-color", color);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Text color", |ui| {
                        for (name, color) in [
                            ("Ink", "#1e304a"),
                            ("Blue", "#285ec4"),
                            ("Green", "#187456"),
                            ("Red", "#b53d45"),
                        ] {
                            if ui.button(name).clicked() {
                                self.apply_style("fo:color", color);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Align", |ui| {
                        for a in ["left", "center", "right"] {
                            if ui.button(a).clicked() {
                                self.apply_style("fo:text-align", a);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Number", |ui| {
                        if ui.button("Currency…").clicked() {
                            self.open_currency_dialog();
                            ui.close();
                        }
                        for label in ["General", "Decimal (2)", "Percent"] {
                            if ui.button(label).clicked() {
                                self.number_format(label);
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    if ui
                        .button("Quick sum")
                        .on_hover_text(
                            "Suggest a SUM for the selected range or numbers above this cell",
                        )
                        .clicked()
                    {
                        self.quick_sum();
                    }
                    if ui.button("Conditional formatting").clicked() {
                        self.commit();
                        self.conditional = true;
                    }
                    if ui.button("Clear style").clicked() {
                        self.commit();
                        self.doc.checkpoint();
                        let (a, b) = self.bounds();
                        for r in a.0..=b.0 {
                            for c in a.1..=b.1 {
                                self.doc
                                    .book
                                    .sheet_mut(self.doc.sheet)
                                    .clear_cellstyle(r, c);
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .selectable_label(
                                self.guide,
                                RichText::new("Guide & functions").color(if self.guide {
                                    Color32::WHITE
                                } else {
                                    INK
                                }),
                            )
                            .clicked()
                        {
                            self.guide = !self.guide;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let jump = ui.add(
                        egui::TextEdit::singleline(&mut self.jump)
                            .id_salt("name_box")
                            .desired_width(72.0)
                            .font(egui::TextStyle::Monospace),
                    );
                    if jump.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        if let Some(p) = formula::coordinate(&self.jump) {
                            self.select(p, false)
                        } else {
                            self.jump = formula::address(self.selected.0, self.selected.1);
                        }
                    }
                    ui.label(RichText::new("ƒx").color(BLUE).size(20.0));
                    let edit = ui.add_sized(
                        [ui.available_width() - 78.0, 30.0],
                        egui::TextEdit::singleline(&mut self.edit)
                            .id_salt("formula_editor")
                            .hint_text("Type a value, or start a formula with ="),
                    );
                    if self.focus_edit {
                        edit.request_focus();
                        self.focus_edit = false;
                    }
                    if edit.changed() {
                        self.editing = true;
                    }
                    if edit.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                        self.editing = false;
                        self.sync_edit();
                        edit.surrender_focus();
                    }
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.commit();
                        self.select((self.selected.0 + 1, self.selected.1), false);
                    }
                    if ui.button("Apply").clicked() {
                        self.commit();
                    }
                });
                if self.editing && self.edit.starts_with('=') {
                    let prefix = self.completion_prefix();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Tab to complete").color(MUTED));
                        for &(name, description, example) in formula::FUNCTIONS {
                            if !prefix.is_empty()
                                && name.starts_with(&prefix)
                                && ui
                                    .button(name)
                                    .on_hover_text(format!("{description}\n={example}"))
                                    .clicked()
                            {
                                self.complete_function(name);
                                self.focus_edit = true;
                            }
                        }
                    });
                }
            });
    }
    fn number_format(&mut self, label: &str) {
        use spreadsheet_ods::{ValueFormatNumber, ValueFormatPercentage};
        self.commit();
        self.doc.checkpoint();
        let format = match label {
            "Decimal (2)" => {
                let mut f = ValueFormatNumber::new_empty();
                f.part_number().fixed_decimal_places(2).build();
                Some(self.doc.book.add_number_format(f))
            }
            "Percent" => {
                let mut f = ValueFormatPercentage::new_empty();
                f.part_number().fixed_decimal_places(1).build();
                f.part_text("%").build();
                Some(self.doc.book.add_percentage_format(f))
            }
            _ => None,
        };
        let (a, b) = self.bounds();
        for r in a.0..=b.0 {
            for c in a.1..=b.1 {
                self.doc.style_cell(r, c, |s| {
                    if let Some(f) = &format {
                        s.set_value_format(f)
                    } else {
                        s.attrmap_mut().clear_attr("style:data-style-name");
                    }
                });
            }
        }
    }
    fn guide_panel(&mut self, ctx: &egui::Context) {
        if !self.guide {
            return;
        }
        egui::SidePanel::right("guide").exact_width(274.0).resizable(false).frame(egui::Frame::new().fill(PALE).inner_margin(20)).show(ctx,|ui|{
            egui::ScrollArea::vertical().show(ui,|ui|{
                ui.label(RichText::new("A HELPING HAND").size(10.0).color(BLUE).strong());ui.add_space(8.0);ui.heading("Small steps.\nUseful sheets.");ui.add_space(10.0);
                let lessons=[("Make yourself at home","Click a cell, then type. Press Enter to apply it. Double-click a cell to edit its existing value."),("Let the sheet do the maths","Try =SUM(B4:B8). A colon means every cell in that range. Change a value and the total updates."),("Make the important bits stand out","Shift-click to select a range. Use bold, fill, number formats, or a conditional formatting rule."),("Take your work with you","Save as .ods, the OpenDocument spreadsheet format. You can open it in LibreOffice. Everything stays on your computer.")];
                let(title,body)=lessons[self.step];
                egui::Frame::new().fill(Color32::WHITE).corner_radius(12).inner_margin(16).show(ui,|ui|{ui.label(RichText::new(format!("{} / 4",self.step+1)).color(BLUE).small());ui.add_space(6.0);ui.label(RichText::new(title).strong().size(16.0));ui.add_space(6.0);ui.label(body);ui.add_space(8.0);ui.horizontal(|ui|{if ui.add_enabled(self.step>0,egui::Button::new("Back")).clicked(){self.step-=1;}
if ui.button(if self.step==3{"Start again"}else{"Next step >"}).clicked(){self.step=(self.step+1)%4;}});});
                ui.add_space(24.0);ui.label(RichText::new("Function library").strong().size(16.0));ui.label(RichText::new("Recipes, without the guesswork.").color(MUTED).size(12.0));ui.add_space(10.0);
                ui.add(egui::TextEdit::singleline(&mut self.function_search).hint_text("Find a function or task…").desired_width(230.0));
                for &(name,description,example)in formula::FUNCTIONS{
                    if !format!("{name} {description}").to_lowercase().contains(&self.function_search.to_lowercase()){continue;}

                    ui.push_id(name,|ui|{egui::CollapsingHeader::new(RichText::new(name).color(BLUE).monospace()).show(ui,|ui|{ui.label(description);ui.label(RichText::new(format!("={example}")).monospace().size(11.0));if ui.button("Use in selected cell").clicked(){self.edit=format!("={example}");self.editing=true;self.focus_edit=true;}});});
                }
                ui.add_space(20.0);ui.separator();ui.add_space(10.0);ui.label(RichText::new("ON YOUR COMPUTER").size(10.0).color(MUTED).strong());ui.label(RichText::new("No accounts. No AI. No cloud.").size(12.0));
                ui.add_space(12.0);ui.label(RichText::new("Keyboard shortcuts").strong());ui.label(RichText::new("Enter / F2  Edit cell\nShift + click  Select range\n⌘ / Ctrl + C, V  Copy, paste\n⌘ / Ctrl + Z  Undo\n⌘ / Ctrl + S  Save").size(11.0).color(MUTED));
            });
        });
    }
    fn grid(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::WHITE))
            .show(ctx, |ui| {
                let used = self.doc.book.sheet(self.doc.sheet).used_grid_size();
                let rows = self
                    .rows
                    .max(used.0 as usize + 20)
                    .max(self.selected.0 as usize + 10)
                    .min(1_048_576);
                let cols = self
                    .cols
                    .max(used.1 as usize + 3)
                    .max(self.selected.1 as usize + 3)
                    .min(16384);
                let w = 124.0;
                let h = 32.0;
                let gutter = 44.0;
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, viewport| {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(gutter + cols as f32 * w, (rows + 1) as f32 * h),
                            egui::Sense::hover(),
                        );
                        let painter = ui.painter();
                        let origin = rect.min;
                        let first_col = ((viewport.min.x - gutter) / w).floor().max(0.0) as usize;
                        let last_col = ((viewport.max.x - gutter) / w).ceil().max(0.0) as usize;
                        let first_row = (viewport.min.y / h).floor().max(1.0) as usize - 1;
                        let last_row = (viewport.max.y / h).ceil().max(1.0) as usize;
                        let (a, b) = self.bounds();
                        let mut select = None;
                        let mut edit = false;
                        // Header strips stay visible as the sheet scrolls.
                        let head_y = origin.y + viewport.min.y;
                        let head_x = origin.x + viewport.min.x;
                        for r in first_row..last_row.min(rows) {
                            for c in first_col..last_col.min(cols) {
                                let cell = egui::Rect::from_min_size(
                                    origin + Vec2::new(gutter + c as f32 * w, (r + 1) as f32 * h),
                                    Vec2::new(w, h),
                                );
                                let selected = r as u32 >= a.0
                                    && r as u32 <= b.0
                                    && c as u32 >= a.1
                                    && c as u32 <= b.1;
                                let style = self.doc.visible_style(r as u32, c as u32);
                                let fill = style
                                    .cellstyle()
                                    .attr("fo:background-color")
                                    .and_then(parse_color)
                                    .unwrap_or(Color32::WHITE);
                                painter.rect_filled(
                                    cell,
                                    0,
                                    if selected {
                                        Color32::from_rgb(230, 239, 254)
                                    } else {
                                        fill
                                    },
                                );
                                painter.rect_stroke(
                                    cell,
                                    0,
                                    Stroke::new(0.5_f32, LINE),
                                    egui::StrokeKind::Inside,
                                );
                                let text = self.doc.formatted(r as u32, c as u32, &style);
                                let numeric = self
                                    .doc
                                    .book
                                    .sheet(self.doc.sheet)
                                    .value(r as u32, c as u32)
                                    .as_f64_opt()
                                    .is_some()
                                    || self
                                        .doc
                                        .book
                                        .sheet(self.doc.sheet)
                                        .formula(r as u32, c as u32)
                                        .is_some();
                                let align = style
                                    .paragraphstyle()
                                    .attr("fo:text-align")
                                    .unwrap_or(if numeric { "right" } else { "left" });
                                let font_size = style
                                    .textstyle()
                                    .attr("fo:font-size")
                                    .and_then(|s| s.trim_end_matches("pt").parse::<f32>().ok())
                                    .unwrap_or(12.0)
                                    .clamp(8.0, 24.0)
                                    * 1.12;
                                let color = self
                                    .doc
                                    .format_color(r as u32, c as u32, &style)
                                    .or_else(|| style.textstyle().attr("fo:color"))
                                    .and_then(parse_color)
                                    .unwrap_or(INK);
                                let bold = style.textstyle().attr("fo:font-weight") == Some("bold");
                                let italic =
                                    style.textstyle().attr("fo:font-style") == Some("italic");
                                let accessible_text =
                                    format!("{}: {}", formula::address(r as u32, c as u32), text);
                                let mut job = egui::text::LayoutJob::simple(
                                    text,
                                    FontId::proportional(font_size),
                                    color,
                                    w - 16.0,
                                );
                                for section in &mut job.sections {
                                    section.format.italics = italic;
                                    if bold {
                                        section.format.font_id = FontId::new(
                                            font_size,
                                            egui::FontFamily::Name("bold".into()),
                                        );
                                    }
                                }
                                job.wrap.max_rows = 1;
                                job.wrap.break_anywhere = true;
                                let galley = painter.layout_job(job);
                                let x = match align {
                                    "right" => cell.right() - 8.0 - galley.size().x,
                                    "center" => cell.center().x - galley.size().x / 2.0,
                                    _ => cell.left() + 8.0,
                                };
                                painter
                                    .with_clip_rect(cell.shrink(3.0).intersect(ui.clip_rect()))
                                    .galley(
                                        egui::pos2(x, cell.center().y - galley.size().y / 2.0),
                                        galley,
                                        color,
                                    );
                                if self.selected == (r as u32, c as u32) {
                                    painter.rect_stroke(
                                        cell.shrink(0.5),
                                        0,
                                        Stroke::new(2.0_f32, BLUE),
                                        egui::StrokeKind::Inside,
                                    );
                                    painter.rect_filled(
                                        egui::Rect::from_center_size(
                                            cell.right_bottom() - Vec2::splat(2.0),
                                            Vec2::splat(5.0),
                                        ),
                                        1,
                                        BLUE,
                                    );
                                }
                                let response = ui.interact(
                                    cell,
                                    ui.id().with((r, c)),
                                    egui::Sense::click_and_drag(),
                                );
                                let response = response.on_hover_text(format!(
                                    "{}\n{}",
                                    formula::address(r as u32, c as u32),
                                    self.doc.raw(r as u32, c as u32)
                                ));
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        &accessible_text,
                                    )
                                });
                                if response.clicked() || response.double_clicked() {
                                    select = Some((r as u32, c as u32));
                                    edit = response.double_clicked();
                                }
                                if response.dragged()
                                    && let Some(pos) = response.interact_pointer_pos()
                                {
                                    let col =
                                        ((pos.x - origin.x - gutter) / w).floor().max(0.0) as u32;
                                    let row = ((pos.y - origin.y) / h).floor().max(1.0) as u32 - 1;
                                    select = Some((row, col));
                                }
                                if self.selected == (r as u32, c as u32)
                                    && ctx.input(|i| {
                                        i.key_pressed(Key::ArrowDown)
                                            || i.key_pressed(Key::ArrowUp)
                                            || i.key_pressed(Key::ArrowLeft)
                                            || i.key_pressed(Key::ArrowRight)
                                            || i.key_pressed(Key::Enter)
                                            || i.key_pressed(Key::Tab)
                                    })
                                {
                                    ui.scroll_to_rect(cell, None);
                                }
                            }
                        }
                        for c in first_col..last_col.min(cols) {
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(origin.x + gutter + c as f32 * w, head_y),
                                Vec2::new(w, h),
                            );
                            painter.rect_filled(rect, 0, PALE);
                            painter.rect_stroke(
                                rect,
                                0,
                                Stroke::new(0.5_f32, LINE),
                                egui::StrokeKind::Inside,
                            );
                            painter.text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                formula::address(0, c as u32).trim_end_matches('1'),
                                FontId::proportional(11.0),
                                MUTED,
                            );
                        }
                        for r in first_row..last_row.min(rows) {
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(head_x, origin.y + (r + 1) as f32 * h),
                                Vec2::new(gutter, h),
                            );
                            painter.rect_filled(rect, 0, PALE);
                            painter.rect_stroke(
                                rect,
                                0,
                                Stroke::new(0.5_f32, LINE),
                                egui::StrokeKind::Inside,
                            );
                            painter.text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                (r + 1).to_string(),
                                FontId::proportional(11.0),
                                MUTED,
                            );
                        }
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(head_x, head_y),
                                Vec2::new(gutter, h),
                            ),
                            0,
                            PALE,
                        );
                        if let Some(p) = select {
                            self.select(
                                p,
                                ctx.input(|i| {
                                    i.modifiers.shift || i.pointer.is_decidedly_dragging()
                                }),
                            );
                            if edit {
                                self.editing = true;
                                self.focus_edit = true;
                            }
                        }
                    });
            });
    }
    fn open_currency_dialog(&mut self) {
        self.commit();
        if let Some(settings) = self
            .doc
            .base_style(self.selected.0, self.selected.1)
            .value_format()
            .and_then(|name| currency::CurrencySettings::from_format(&self.doc.book, name))
        {
            self.currency_settings = settings;
        }
        self.currency_open = true;
    }
    fn currency_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("Currency format")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Format {} as currency", self.range_label()));
                ui.add_space(8.0);
                egui::Grid::new("currency_options")
                    .num_columns(2)
                    .spacing([24.0, 12.0])
                    .show(ui, |ui| {
                        ui.label("Currency symbol");
                        egui::ComboBox::from_id_salt("currency_symbol")
                            .selected_text(self.currency_settings.label())
                            .show_ui(ui, |ui| {
                                for (i, (code, symbol, _)) in
                                    currency::CURRENCIES.iter().enumerate()
                                {
                                    ui.selectable_value(
                                        &mut self.currency_settings.currency,
                                        i,
                                        format!("{code} ({symbol})"),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label("Decimal places");
                        ui.add(
                            egui::DragValue::new(&mut self.currency_settings.decimals).range(0..=6),
                        );
                        ui.end_row();
                        ui.label("Thousands separator");
                        ui.checkbox(&mut self.currency_settings.grouping, "Group digits (1,000)");
                        ui.end_row();
                        ui.label("Negative amounts");
                        egui::ComboBox::from_id_salt("currency_negative")
                            .selected_text(self.currency_settings.negative.label())
                            .show_ui(ui, |ui| {
                                for mode in currency::Negative::ALL {
                                    ui.selectable_value(
                                        &mut self.currency_settings.negative,
                                        mode,
                                        mode.label(),
                                    );
                                }
                            });
                        ui.end_row();
                    });
                ui.add_space(12.0);
                egui::Frame::new()
                    .fill(PALE)
                    .corner_radius(8)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Preview").small().color(MUTED));
                        ui.horizontal(|ui| {
                            ui.label(self.currency_settings.preview(1234.567));
                            ui.label(
                                RichText::new(self.currency_settings.preview(-1234.567)).color(
                                    if self.currency_settings.negative.red() {
                                        Color32::from_rgb(255, 0, 0)
                                    } else {
                                        INK
                                    },
                                ),
                            );
                        });
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Changes how numbers look; keeps their original values.")
                        .small()
                        .color(MUTED),
                );
                ui.horizontal(|ui| {
                    if ui.button("Apply currency").clicked() {
                        let (a, b) = self.bounds();
                        self.doc.apply_currency(a, b, &self.currency_settings);
                        self.currency_open = false;
                        self.status =
                            format!("Applied {} currency format", self.currency_settings.label());
                    }
                    if ui.button("Cancel").clicked() {
                        self.currency_open = false;
                    }
                });
            });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm {
            egui::Window::new("Save your changes?")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("This workbook has changes that haven’t been saved.");
                    ui.horizontal(|ui| {
                        if ui.button("Save changes").clicked() && self.save(false) {
                            self.confirm = false;
                            if let Some(p) = self.pending.take() {
                                self.execute(p, ctx)
                            }
                        }
                        if ui.button("Discard changes").clicked() {
                            self.confirm = false;
                            if let Some(p) = self.pending.take() {
                                self.execute(p, ctx)
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm = false;
                            self.pending = None;
                        }
                    });
                });
        }
        if self.currency_open {
            self.currency_dialog(ctx);
        }
        if self.conditional {
            egui::Window::new("Conditional formatting")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Highlight {} when its value is…",
                        self.range_label()
                    ));
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("operator")
                            .selected_text(&self.operator)
                            .show_ui(ui, |ui| {
                                for op in ["<", ">", "=", "<=", ">=", "!="] {
                                    ui.selectable_value(&mut self.operator, op.into(), op);
                                }
                            });
                        ui.add(egui::DragValue::new(&mut self.threshold).speed(1.0));
                    });
                    ui.label("Matching cells receive a soft rose fill.");
                    ui.horizontal(|ui| {
                        if ui.button("Apply rule").clicked() {
                            self.doc.checkpoint();
                            let (a, b) = self.bounds();
                            self.doc
                                .conditional(a, b, &self.operator, self.threshold, "#ffe1de");
                            self.conditional = false;
                            self.status = "Conditional rule applied".into();
                        }
                        if ui.button("Cancel").clicked() {
                            self.conditional = false;
                        }
                    });
                });
        }
        if self.rename_open {
            egui::Window::new("Rename sheet")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.text_edit_singleline(&mut self.rename);
                    ui.horizontal(|ui| {
                        let valid = !self.rename.trim().is_empty()
                            && !self
                                .doc
                                .book
                                .iter_sheets()
                                .any(|s| s.name() == self.rename.trim());
                        if ui.add_enabled(valid, egui::Button::new("Rename")).clicked() {
                            self.doc.checkpoint();
                            self.doc
                                .book
                                .sheet_mut(self.doc.sheet)
                                .set_name(self.rename.trim());
                            self.rename_open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.rename_open = false;
                        }
                    });
                });
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Workbook needs attention")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(480.0);
                    ui.label(error);
                    if ui.button("Got it").clicked() {
                        self.error = None;
                    }
                });
        }
    }
}
fn parse_color(s: &str) -> Option<Color32> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}
impl eframe::App for EasySheet {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.allow_close
            && ctx.input(|i| i.viewport().close_requested())
            && (self.doc.dirty || self.editing)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Pending::Close, ctx);
        }
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()))
        {
            self.commit();
            if self.doc.dirty {
                self.error=Some("Save your current changes before dropping another workbook, or use Open to choose whether to discard them.".into());
            } else {
                self.load(path);
            }
        }
        self.keyboard(ctx);
        self.header(ctx);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(PALE)
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("LOCAL")
                            .color(Color32::from_rgb(35, 140, 102))
                            .size(10.0),
                    );
                    ui.label(RichText::new(&self.status).size(11.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}  ·  100%  ·  Local", self.range_label()))
                                .size(11.0)
                                .color(MUTED),
                        );
                        let (a, b) = self.bounds();
                        if (b.0 - a.0 + 1) as u64 * (b.1 - a.1 + 1) as u64 <= 10000 {
                            let mut sum = 0.0;
                            let mut count = 0;
                            for r in a.0..=b.0 {
                                for c in a.1..=b.1 {
                                    if !self.doc.raw(r, c).is_empty()
                                        && let Ok(v) =
                                            formula::evaluate(&self.doc.book, self.doc.sheet, r, c)
                                    {
                                        sum += v;
                                        count += 1;
                                    }
                                }
                            }
                            if count > 1 {
                                ui.label(
                                    RichText::new(format!("Sum {sum:.2}  ·  Count {count}"))
                                        .size(11.0),
                                );
                            }
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("tabs")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let names: Vec<_> = self
                        .doc
                        .book
                        .iter_sheets()
                        .map(|s| s.name().clone())
                        .collect();
                    for (i, name) in names.iter().enumerate() {
                        let response = ui.selectable_label(
                            self.doc.sheet == i,
                            RichText::new(name).strong().color(if self.doc.sheet == i {
                                Color32::WHITE
                            } else {
                                INK
                            }),
                        );
                        if response.clicked() {
                            self.commit();
                            self.doc.sheet = i;
                            self.selected = (0, 0);
                            self.anchor = (0, 0);
                            self.sync_edit();
                        }
                        if response.double_clicked() {
                            self.rename = name.clone();
                            self.rename_open = true;
                        }
                    }
                    if ui.button("+").on_hover_text("Add sheet").clicked() {
                        self.commit();
                        self.doc.checkpoint();
                        let mut n = self.doc.book.num_sheets() + 1;
                        while self.doc.book.sheet_idx(format!("Sheet {n}")).is_some() {
                            n += 1;
                        }
                        self.doc
                            .book
                            .push_sheet(spreadsheet_ods::Sheet::new(format!("Sheet {n}")));
                        self.doc.sheet = self.doc.book.num_sheets() - 1;
                        self.select((0, 0), false);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Double-click a tab to rename")
                                .color(MUTED)
                                .size(10.0),
                        );
                    });
                });
            });
        self.guide_panel(ctx);
        self.grid(ctx);
        self.dialogs(ctx);
    }
}
fn main() -> eframe::Result {
    if let Some(path) = std::env::args().nth(1).filter(|s| s == "--export-example") {
        let _ = path;
        let output = std::env::args().nth(2).unwrap_or("example.ods".into());
        Document::demo()
            .save(std::path::Path::new(&output))
            .expect("export example");
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([900.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
        "EasySheet",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Lato".into(),
                egui::FontData::from_static(include_bytes!("../assets/fonts/Lato-Regular.ttf"))
                    .into(),
            );
            fonts.font_data.insert(
                "Lato Bold".into(),
                egui::FontData::from_static(include_bytes!("../assets/fonts/Lato-Bold.ttf")).into(),
            );
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "Lato".into());
            let mut proportional = fonts.families[&egui::FontFamily::Proportional].clone();
            proportional.insert(0, "Lato Bold".into());
            fonts
                .families
                .insert(egui::FontFamily::Name("bold".into()), proportional);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(EasySheet::new(cc)))
        }),
    )
}
