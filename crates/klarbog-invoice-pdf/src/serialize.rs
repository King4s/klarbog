//! PDF 1.4 object serialization (deterministic, integer geometry).

use crate::draw::{
    fmt_cp, fmt_gray100, DrawOp, PageWriter, CONTENT_RIGHT_CP, MARGIN_X_CP, PAGE_BOTTOM_CP,
    PAGE_HEIGHT_CP, PAGE_WIDTH_CP,
};
use crate::encode::{compact, escape_pdf_text};
use crate::layout::layout_invoice;
use crate::payload::IssuedInvoicePdfPayload;

fn page_content_stream(page: &PageWriter, footer: &str) -> String {
    let mut parts = Vec::new();
    parts.push(format!(
        "BT /F1 {} Tf {} g 1 0 0 1 {} {} Tm ({}) Tj ET",
        fmt_cp(750),
        fmt_gray100(50),
        fmt_cp(MARGIN_X_CP),
        fmt_cp(PAGE_BOTTOM_CP - 2200),
        escape_pdf_text(footer)
    ));
    parts.push(format!(
        "{} g {} {} m {} {} l {} w S",
        fmt_gray100(85),
        fmt_cp(MARGIN_X_CP),
        fmt_cp(PAGE_BOTTOM_CP - 1000),
        fmt_cp(CONTENT_RIGHT_CP),
        fmt_cp(PAGE_BOTTOM_CP - 1000),
        fmt_cp(50)
    ));
    for op in &page.ops {
        match op {
            DrawOp::Rect {
                x_cp,
                y_cp,
                w_cp,
                h_cp,
                gray100,
            } => {
                parts.push(format!(
                    "{} g {} {} {} {} re f",
                    fmt_gray100(*gray100),
                    fmt_cp(*x_cp),
                    fmt_cp(*y_cp),
                    fmt_cp(*w_cp),
                    fmt_cp(*h_cp)
                ));
            }
            DrawOp::Line {
                x1_cp,
                y1_cp,
                x2_cp,
                y2_cp,
                gray100,
                width_cp,
            } => {
                parts.push(format!(
                    "{} G {} w {} {} m {} {} l S",
                    fmt_gray100(*gray100),
                    fmt_cp(*width_cp),
                    fmt_cp(*x1_cp),
                    fmt_cp(*y1_cp),
                    fmt_cp(*x2_cp),
                    fmt_cp(*y2_cp)
                ));
            }
            DrawOp::Text {
                x_cp,
                y_cp,
                size_cp,
                font,
                gray100,
                text,
            } => {
                parts.push(format!(
                    "BT /{} {} Tf {} g 1 0 0 1 {} {} Tm ({}) Tj ET",
                    font.as_pdf(),
                    fmt_cp(*size_cp),
                    fmt_gray100(*gray100),
                    fmt_cp(*x_cp),
                    fmt_cp(*y_cp),
                    escape_pdf_text(text)
                ));
            }
        }
    }
    format!("{}\n", parts.join("\n"))
}

fn binary_len(s: &str) -> usize {
    s.len()
}

/// Render an issued invoice into a deterministic A4 PDF 1.4 byte buffer.
pub fn build_issued_invoice_pdf(payload: &IssuedInvoicePdfPayload) -> Vec<u8> {
    let pages = layout_invoice(payload);
    let issue_date = compact(payload.issue_date.as_deref()).unwrap_or_else(|| "1970-01-01".into());
    let pdf_date = format!("D:{}000000Z", issue_date.replace('-', ""));
    let producer = "Klarbog deterministic invoice renderer";
    let title = format!(
        "Faktura {}",
        compact(payload.invoice_number.as_deref()).unwrap_or_default()
    )
    .trim()
    .to_string();

    let page_count = pages.len();
    let page_object_nos: Vec<usize> = (0..page_count).map(|i| 3 + i * 2).collect();
    let font_regular_no = 3 + page_count * 2;
    let font_bold_no = font_regular_no + 1;
    let info_object_no = font_bold_no + 1;

    let mut objects: Vec<String> = Vec::new();
    objects.push("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".into());
    let kids = page_object_nos
        .iter()
        .map(|n| format!("{n} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(format!(
        "2 0 obj\n<< /Type /Pages /Count {page_count} /Kids [{kids}] >>\nendobj\n"
    ));

    for (i, page) in pages.iter().enumerate() {
        let page_no = page_object_nos[i];
        let content_no = page_no + 1;
        let footer = format!("{} - Side {} af {page_count}", title, i + 1);
        let content = page_content_stream(page, &footer);
        objects.push(format!(
            "{page_no} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] \
             /Resources << /Font << /F1 {font_regular_no} 0 R /F2 {font_bold_no} 0 R >> >> \
             /Contents {content_no} 0 R >>\nendobj\n",
            fmt_cp(PAGE_WIDTH_CP),
            fmt_cp(PAGE_HEIGHT_CP)
        ));
        objects.push(format!(
            "{content_no} 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n",
            binary_len(&content)
        ));
    }

    objects.push(format!(
        "{font_regular_no} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
         /Encoding /WinAnsiEncoding >>\nendobj\n"
    ));
    objects.push(format!(
        "{font_bold_no} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold \
         /Encoding /WinAnsiEncoding >>\nendobj\n"
    ));
    objects.push(format!(
        "{info_object_no} 0 obj\n<< /Producer ({}) /Title ({}) \
         /CreationDate ({pdf_date}) /ModDate ({pdf_date}) >>\nendobj\n",
        escape_pdf_text(producer),
        escape_pdf_text(&title)
    ));

    let mut pdf = String::from("%PDF-1.4\n%\u{00e2}\u{00e3}\u{00cf}\u{00d3}\n");
    let mut offsets = vec![0usize];
    for object in &objects {
        offsets.push(binary_len(&pdf));
        pdf.push_str(object);
    }
    let xref_offset = binary_len(&pdf);
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R /Info {info_object_no} 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
        objects.len() + 1
    ));
    pdf.into_bytes()
}
