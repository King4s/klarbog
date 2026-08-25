//! Invoice page layout orchestration (header + parties). Centipoint geometry.

#[path = "layout_body.rs"]
mod layout_body;

use crate::draw::{
    FontName, PageWriter, CONTENT_RIGHT_CP, LINE_HEIGHT_CP, MARGIN_X_CP, PAGE_TOP_CP,
};
use crate::encode::compact;
use crate::metrics::{fit_text, right_align_x_cp};
use crate::payload::IssuedInvoicePdfPayload;
use layout_body::{layout_table, layout_totals_and_notes};

fn party_lines(party: Option<&crate::payload::PdfParty>) -> Vec<String> {
    let Some(party) = party else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(n) = compact(party.name.as_deref()) {
        out.push(n);
    }
    if let Some(addr) = compact(party.address.as_deref()) {
        for part in addr.split(['\r', '\n']) {
            if !part.trim().is_empty() {
                out.push(part.trim().to_string());
            }
        }
    }
    if let Some(cvr) = compact(party.vat_or_cvr.as_deref()) {
        out.push(format!("CVR/SE: {cvr}"));
    }
    out
}

fn current(pages: &mut [PageWriter]) -> &mut PageWriter {
    pages.last_mut().expect("at least one page")
}

/// Lay the whole invoice out across one or more A4 pages.
pub fn layout_invoice(payload: &IssuedInvoicePdfPayload) -> Vec<PageWriter> {
    let currency = payload
        .currency
        .as_deref()
        .unwrap_or("DKK")
        .trim()
        .to_uppercase();
    let mut pages: Vec<PageWriter> = vec![PageWriter::new()];
    layout_header(payload, &currency, &mut pages);
    layout_table(payload, &mut pages);
    layout_totals_and_notes(payload, &currency, &mut pages);
    pages
}

fn layout_header(payload: &IssuedInvoicePdfPayload, currency: &str, pages: &mut [PageWriter]) {
    let page = current(pages);
    let seller_name = compact(payload.seller.as_ref().and_then(|s| s.name.as_deref()));
    let brand = compact(payload.logo_text.as_deref())
        .or(seller_name)
        .unwrap_or_else(|| "Faktura".into());
    page.text_at(MARGIN_X_CP, PAGE_TOP_CP, &brand, 2000, FontName::F2, 0);
    let faktura = "FAKTURA";
    page.text_at(
        right_align_x_cp(faktura, 2200, CONTENT_RIGHT_CP),
        PAGE_TOP_CP + 200,
        faktura,
        2200,
        FontName::F2,
        15,
    );

    let mut meta: Vec<(String, String)> = Vec::new();
    if let Some(n) = compact(payload.invoice_number.as_deref()) {
        meta.push(("Fakturanr.".into(), n));
    }
    if let Some(d) = compact(payload.issue_date.as_deref()) {
        meta.push(("Fakturadato".into(), d));
    }
    if let Some(d) = compact(payload.due_date.as_deref()) {
        meta.push(("Forfaldsdato".into(), d));
    }
    if let Some(d) = compact(payload.delivery_date.as_deref()) {
        meta.push(("Leveringsdato".into(), d));
    } else if let (Some(a), Some(b)) = (
        compact(payload.delivery_period_start.as_deref()),
        compact(payload.delivery_period_end.as_deref()),
    ) {
        meta.push(("Leveringsperiode".into(), format!("{a} – {b}")));
    }
    meta.push(("Valuta".into(), currency.to_string()));

    let mut meta_y = PAGE_TOP_CP - 2600;
    for (label, value) in &meta {
        page.text_at(
            right_align_x_cp(label, 800, CONTENT_RIGHT_CP - 13000),
            meta_y,
            label,
            800,
            FontName::F1,
            45,
        );
        page.text_at(
            right_align_x_cp(value, 950, CONTENT_RIGHT_CP),
            meta_y,
            value,
            950,
            FontName::F2,
            0,
        );
        meta_y -= 1300;
    }

    let header_bottom = (PAGE_TOP_CP - 3600).min(meta_y) - 400;
    page.hline(header_bottom, 70, 100, MARGIN_X_CP, CONTENT_RIGHT_CP);
    page.y_cp = header_bottom - 2200;

    let col_width = (CONTENT_RIGHT_CP - MARGIN_X_CP - 2400) / 2;
    let buyer_x = MARGIN_X_CP + col_width + 2400;
    let block_top = page.y_cp;
    let seller_bottom = draw_party(
        page,
        MARGIN_X_CP,
        "SÆLGER",
        &party_lines(payload.seller.as_ref()),
        col_width,
        block_top,
    );
    let buyer_bottom = draw_party(
        page,
        buyer_x,
        "KØBER",
        &party_lines(payload.buyer.as_ref()),
        col_width,
        block_top,
    );
    page.y_cp = seller_bottom.min(buyer_bottom) - 1800;
}

fn draw_party(
    page: &mut PageWriter,
    x_cp: i32,
    heading: &str,
    lines: &[String],
    col_width_cp: i32,
    block_top_cp: i32,
) -> i32 {
    let mut yy = block_top_cp;
    page.text_at(x_cp, yy, heading, 850, FontName::F2, 45);
    yy -= 1600;
    if lines.is_empty() {
        page.text_at(x_cp, yy, "—", 1000, FontName::F1, 50);
        return yy - LINE_HEIGHT_CP;
    }
    for (i, line) in lines.iter().enumerate() {
        let size = if i == 0 { 1100 } else { 950 };
        let font = if i == 0 { FontName::F2 } else { FontName::F1 };
        let gray = if i == 0 { 0 } else { 25 };
        page.text_at(
            x_cp,
            yy,
            &fit_text(line, size, col_width_cp),
            size,
            font,
            gray,
        );
        yy -= if i == 0 { 1600 } else { LINE_HEIGHT_CP };
    }
    yy
}
