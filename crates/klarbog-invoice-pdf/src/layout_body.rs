//! Line-item table, totals, payment, and notes (centipoint geometry).

use crate::draw::{
    FontName, PageWriter, CONTENT_RIGHT_CP, LINE_HEIGHT_CP, MARGIN_X_CP, PAGE_TOP_CP,
};
use crate::encode::compact;
use crate::metrics::{fit_text, right_align_x_cp, wrap_text};
use crate::money_fmt::{
    format_danish_dkk_minor, format_danish_minor, format_fx_micro, format_quantity_milli,
};
use crate::payload::{IssuedInvoicePdfPayload, PdfLine, PdfPayment, PdfTotals};

struct LineItem {
    description: String,
    quantity: String,
    unit_price: String,
    line_total: String,
}

fn line_item_from(line: &PdfLine) -> LineItem {
    let mut desc = line.description.clone();
    if let Some(tax) = compact(line.tax_classification.as_deref()) {
        desc.push_str(&format!(" [{tax}]"));
    }
    let line_total = line
        .line_total_minor
        .or(line.amount_minor)
        .map(format_danish_minor)
        .unwrap_or_default();
    LineItem {
        description: desc,
        quantity: line
            .quantity_milli
            .map(format_quantity_milli)
            .unwrap_or_default(),
        unit_price: line
            .unit_price_minor
            .map(format_danish_minor)
            .unwrap_or_default(),
        line_total,
    }
}

fn payment_lines(payment: Option<&PdfPayment>) -> Vec<String> {
    let Some(payment) = payment else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if let Some(bank) = compact(payment.bank_name.as_deref()) {
        lines.push(bank);
    }
    let reg = compact(payment.registration_no.as_deref());
    let acct = compact(payment.account_no.as_deref());
    match (&reg, &acct) {
        (Some(r), Some(a)) => lines.push(format!("Indenlandsk: Reg.nr. {r}  Kontonr. {a}")),
        (None, Some(a)) => lines.push(format!("Kontonr. {a}")),
        (Some(r), None) => lines.push(format!("Reg.nr. {r}")),
        _ => {}
    }
    if let Some(iban) = compact(payment.iban.as_deref()) {
        lines.push(format!("International: IBAN: {iban}"));
    }
    if let Some(bic) = compact(payment.bic.as_deref()) {
        lines.push(format!("SWIFT/BIC: {bic}"));
    }
    if let Some(owner) = compact(payment.account_owner.as_deref()) {
        lines.push(format!("Kontoejer: {owner}"));
    }
    if let Some(c) = compact(payment.customer_no.as_deref()) {
        lines.push(format!("Bank-kundenr.: {c}"));
    }
    lines
}

fn current(pages: &mut [PageWriter]) -> &mut PageWriter {
    pages.last_mut().expect("at least one page")
}

fn ensure_room(pages: &mut Vec<PageWriter>, needed_cp: i32) {
    if !current(pages).has_room(needed_cp) {
        pages.push(PageWriter::new());
        current(pages).y_cp = PAGE_TOP_CP;
    }
}

pub(crate) fn layout_table(payload: &IssuedInvoicePdfPayload, pages: &mut Vec<PageWriter>) {
    let col_total_right = CONTENT_RIGHT_CP;
    let col_unit_right = col_total_right - 9600;
    let col_qty_right = col_unit_right - 7000;
    let desc_x = MARGIN_X_CP + 600;
    let desc_max = col_qty_right - 6000 - desc_x;
    let table_header_height = 2000;
    let row_height = 2000;

    let draw_header = |page: &mut PageWriter| {
        let top = page.y_cp;
        page.rect(
            MARGIN_X_CP,
            top - table_header_height,
            CONTENT_RIGHT_CP - MARGIN_X_CP,
            table_header_height,
            92,
        );
        let label_y = top - 1400;
        page.text_at(desc_x, label_y, "Beskrivelse", 850, FontName::F2, 35);
        page.text_at(
            right_align_x_cp("Antal", 850, col_qty_right),
            label_y,
            "Antal",
            850,
            FontName::F2,
            35,
        );
        page.text_at(
            right_align_x_cp("Stykpris", 850, col_unit_right),
            label_y,
            "Stykpris",
            850,
            FontName::F2,
            35,
        );
        page.text_at(
            right_align_x_cp("Beløb", 850, col_total_right),
            label_y,
            "Beløb",
            850,
            FontName::F2,
            35,
        );
        page.hline(
            top - table_header_height,
            55,
            75,
            MARGIN_X_CP,
            CONTENT_RIGHT_CP,
        );
        page.y_cp = top - table_header_height;
    };

    draw_header(current(pages));
    let items: Vec<LineItem> = payload.lines.iter().map(line_item_from).collect();
    for (index, item) in items.iter().enumerate() {
        if !current(pages).has_room(row_height + 400) {
            pages.push(PageWriter::new());
            current(pages).y_cp = PAGE_TOP_CP;
            draw_header(current(pages));
        }
        let page = current(pages);
        let row_top = page.y_cp;
        if index % 2 == 1 {
            page.rect(
                MARGIN_X_CP,
                row_top - row_height,
                CONTENT_RIGHT_CP - MARGIN_X_CP,
                row_height,
                97,
            );
        }
        let cell_y = row_top - 1400;
        let desc = if item.description.is_empty() {
            "—"
        } else {
            item.description.as_str()
        };
        page.text_at(
            desc_x,
            cell_y,
            &fit_text(desc, 950, desc_max),
            950,
            FontName::F1,
            10,
        );
        if !item.quantity.is_empty() {
            page.text_at(
                right_align_x_cp(&item.quantity, 950, col_qty_right),
                cell_y,
                &item.quantity,
                950,
                FontName::F1,
                10,
            );
        }
        if !item.unit_price.is_empty() {
            page.text_at(
                right_align_x_cp(&item.unit_price, 950, col_unit_right),
                cell_y,
                &item.unit_price,
                950,
                FontName::F1,
                10,
            );
        }
        if !item.line_total.is_empty() {
            page.text_at(
                right_align_x_cp(&item.line_total, 950, col_total_right),
                cell_y,
                &item.line_total,
                950,
                FontName::F2,
                0,
            );
        }
        page.hline(row_top - row_height, 90, 40, MARGIN_X_CP, CONTENT_RIGHT_CP);
        page.y_cp = row_top - row_height;
    }
}

fn total_rows(totals: Option<&PdfTotals>, currency: &str) -> Vec<(String, String, bool)> {
    let Some(t) = totals else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    if let Some(net) = t.net_minor {
        rows.push((
            "Netto".into(),
            format_danish_dkk_minor(net, currency),
            false,
        ));
    }
    if let Some(bps) = t.vat_rate_bps {
        if let Some(vat) = t.vat_minor {
            let whole = bps / 100;
            let frac = bps.rem_euclid(100);
            let label = if frac == 0 {
                format!("Moms ({whole}%)")
            } else {
                format!("Moms ({whole}.{frac:02}%)")
            };
            rows.push((label, format_danish_dkk_minor(vat, currency), false));
        }
    } else if let Some(vat) = t.vat_minor {
        rows.push(("Moms".into(), format_danish_dkk_minor(vat, currency), false));
    }
    if let Some(gross) = t.gross_minor {
        rows.push((
            "Total".into(),
            format_danish_dkk_minor(gross, currency),
            true,
        ));
    }
    if currency != "DKK" {
        if let Some(fx) = t.fx_rate_to_dkk_micro {
            rows.push(("Valutakurs til DKK".into(), format_fx_micro(fx), false));
        }
        if let Some(g) = t.gross_amount_dkk_minor {
            rows.push((
                "Total i DKK".into(),
                format_danish_dkk_minor(g, "DKK"),
                true,
            ));
        }
    }
    rows
}

pub(crate) fn layout_totals_and_notes(
    payload: &IssuedInvoicePdfPayload,
    currency: &str,
    pages: &mut Vec<PageWriter>,
) {
    let rows = total_rows(payload.totals.as_ref(), currency);
    let totals_height = (rows.len() as i32) * 1600 + 1400;
    if !current(pages).has_room(totals_height + 1000) {
        pages.push(PageWriter::new());
        current(pages).y_cp = PAGE_TOP_CP;
    } else {
        current(pages).advance(1200);
    }

    let col_width = (CONTENT_RIGHT_CP - MARGIN_X_CP - 2400) / 2;
    let totals_label_right = CONTENT_RIGHT_CP - 12000;
    let totals_block_left = MARGIN_X_CP + col_width + 2400;
    for (label, value, emphasis) in &rows {
        let page = current(pages);
        if *emphasis {
            page.rect(
                totals_block_left,
                page.y_cp - 1700,
                CONTENT_RIGHT_CP - totals_block_left,
                1900,
                92,
            );
            page.y_cp -= 400;
        }
        let size = if *emphasis { 1100 } else { 950 };
        let font = if *emphasis {
            FontName::F2
        } else {
            FontName::F1
        };
        let gray = if *emphasis { 0 } else { 40 };
        let y = page.y_cp - 800;
        page.text_at(
            right_align_x_cp(label, size, totals_label_right),
            y,
            label,
            size,
            font,
            gray,
        );
        page.text_at(
            right_align_x_cp(value, size, CONTENT_RIGHT_CP),
            y,
            value,
            size,
            FontName::F2,
            0,
        );
        page.y_cp -= 1600;
    }

    let pay = payment_lines(payload.payment.as_ref());
    if !pay.is_empty() {
        let block_height = (pay.len() as i32) * LINE_HEIGHT_CP + 2600;
        ensure_room(pages, block_height + 800);
        let page = current(pages);
        if page.y_cp < PAGE_TOP_CP - 100 {
            page.advance(2200);
        }
        page.hline(page.y_cp, 85, 50, MARGIN_X_CP, CONTENT_RIGHT_CP);
        page.advance(1600);
        page.text(MARGIN_X_CP, "BETALING", 850, FontName::F2, 45);
        page.advance(1600);
        for line in &pay {
            page.text(MARGIN_X_CP, line, 950, FontName::F1, 15);
            page.advance(LINE_HEIGHT_CP);
        }
        if let Some(n) = compact(payload.invoice_number.as_deref()) {
            page.text(
                MARGIN_X_CP,
                &format!("Anfør fakturanr. {n} ved betaling."),
                850,
                FontName::F1,
                45,
            );
            page.advance(LINE_HEIGHT_CP);
        }
    }

    if let Some(note) = compact(payload.reverse_charge_note.as_deref()) {
        ensure_room(pages, 4000);
        {
            let page = current(pages);
            if page.y_cp < PAGE_TOP_CP - 100 {
                page.advance(1400);
            }
            page.text(MARGIN_X_CP, "NOTE", 850, FontName::F2, 45);
            page.advance(1400);
        }
        for wrapped in wrap_text(&note, 900, CONTENT_RIGHT_CP - MARGIN_X_CP) {
            ensure_room(pages, LINE_HEIGHT_CP);
            let page = current(pages);
            page.text(MARGIN_X_CP, &wrapped, 900, FontName::F1, 20);
            page.advance(LINE_HEIGHT_CP);
        }
    }
}
