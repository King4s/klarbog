//! Page geometry in centipoints (1/100 PDF point). Gray is 0..=100.

pub const PAGE_WIDTH_CP: i32 = 59500;
pub const PAGE_HEIGHT_CP: i32 = 84200;
pub const MARGIN_X_CP: i32 = 5600;
pub const CONTENT_RIGHT_CP: i32 = PAGE_WIDTH_CP - MARGIN_X_CP;
pub const PAGE_TOP_CP: i32 = PAGE_HEIGHT_CP - 5600;
pub const PAGE_BOTTOM_CP: i32 = 7000;
pub const LINE_HEIGHT_CP: i32 = 1400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontName {
    F1,
    F2,
}

impl FontName {
    pub fn as_pdf(self) -> &'static str {
        match self {
            Self::F1 => "F1",
            Self::F2 => "F2",
        }
    }
}

#[derive(Debug, Clone)]
pub enum DrawOp {
    Text {
        x_cp: i32,
        y_cp: i32,
        size_cp: i32,
        font: FontName,
        gray100: i32,
        text: String,
    },
    Rect {
        x_cp: i32,
        y_cp: i32,
        w_cp: i32,
        h_cp: i32,
        gray100: i32,
    },
    Line {
        x1_cp: i32,
        y1_cp: i32,
        x2_cp: i32,
        y2_cp: i32,
        gray100: i32,
        width_cp: i32,
    },
}

#[derive(Debug, Clone)]
pub struct PageWriter {
    pub ops: Vec<DrawOp>,
    pub y_cp: i32,
}

impl Default for PageWriter {
    fn default() -> Self {
        Self {
            ops: Vec::new(),
            y_cp: PAGE_TOP_CP,
        }
    }
}

impl PageWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_room(&self, needed_cp: i32) -> bool {
        self.y_cp - needed_cp >= PAGE_BOTTOM_CP
    }

    pub fn text_at(
        &mut self,
        x_cp: i32,
        y_cp: i32,
        text: &str,
        size_cp: i32,
        font: FontName,
        gray100: i32,
    ) {
        self.ops.push(DrawOp::Text {
            x_cp,
            y_cp,
            size_cp,
            font,
            gray100,
            text: text.to_string(),
        });
    }

    pub fn text(&mut self, x_cp: i32, text: &str, size_cp: i32, font: FontName, gray100: i32) {
        let y = self.y_cp;
        self.text_at(x_cp, y, text, size_cp, font, gray100);
    }

    pub fn rect(&mut self, x_cp: i32, y_cp: i32, w_cp: i32, h_cp: i32, gray100: i32) {
        self.ops.push(DrawOp::Rect {
            x_cp,
            y_cp,
            w_cp,
            h_cp,
            gray100,
        });
    }

    pub fn hline(&mut self, y_cp: i32, gray100: i32, width_cp: i32, x1_cp: i32, x2_cp: i32) {
        self.ops.push(DrawOp::Line {
            x1_cp,
            y1_cp: y_cp,
            x2_cp,
            y2_cp: y_cp,
            gray100,
            width_cp,
        });
    }

    pub fn advance(&mut self, amount_cp: i32) {
        self.y_cp -= amount_cp;
    }
}

/// Centipoints → deterministic PDF number string.
pub fn fmt_cp(value_cp: i32) -> String {
    if value_cp % 100 == 0 {
        format!("{}", value_cp / 100)
    } else {
        let neg = value_cp < 0;
        let abs = value_cp.unsigned_abs();
        format!(
            "{}{}.{:02}",
            if neg { "-" } else { "" },
            abs / 100,
            abs % 100
        )
    }
}

/// Gray 0..=100 → PDF gray operator operand.
pub fn fmt_gray100(g: i32) -> String {
    let g = g.clamp(0, 100);
    if g == 0 {
        "0".into()
    } else if g == 100 {
        "1".into()
    } else {
        format!("0.{g:02}")
    }
}
