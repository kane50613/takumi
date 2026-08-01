//! Renders showcase PDFs (an invoice and a certificate) from HTML markup.
//!
//! ```sh
//! cargo run -p takumi-pdf --example showcase
//! ```

use std::{fs, path::Path};

use takumi_core::{Fonts, layout::node::Node, resources::font::FontResource, viewport::Viewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_pdf::{PageOptions, PdfOptions, render};

fn main() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
  let mut fonts = Fonts::default();
  let data =
    fs::read(root.join("assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf")).unwrap();

  fonts.register(FontResource::new(data)).unwrap();

  let invoice = render(
    PdfOptions::builder()
      .node(invoice())
      .viewport(Viewport::new((794, 1123)))
      .page(PageOptions::a4().with_margin(36.0))
      .footer(html(INVOICE_FOOTER))
      .fonts(&fonts)
      .build(),
  )
  .unwrap();

  fs::write(root.join("target/showcase-invoice.pdf"), invoice).unwrap();

  let certificate = render(
    PdfOptions::builder()
      .node(html(CERTIFICATE))
      .viewport(Viewport::new((1123, 794)))
      .fonts(&fonts)
      .build(),
  )
  .unwrap();

  fs::write(root.join("target/showcase-certificate.pdf"), certificate).unwrap();
  println!("wrote target/showcase-invoice.pdf and target/showcase-certificate.pdf");
}

fn html(source: &str) -> Node {
  from_html(source, FromHtmlOptions::default()).unwrap()
}

struct Line {
  pos: u32,
  count: u32,
  product: &'static str,
  detail: Option<&'static str>,
  unit: &'static str,
  quantity: f32,
  unit_price: f32,
}

struct Project {
  name: &'static str,
  period: &'static str,
  lines: Vec<Line>,
}

fn projects() -> Vec<Project> {
  let mut pos = 0;
  let mut line = |count, product, detail, unit, quantity, unit_price| {
    pos += 1;
    Line {
      pos,
      count,
      product,
      detail,
      unit,
      quantity,
      unit_price,
    }
  };

  vec![
    Project {
      name: "atlas",
      period: "07/2026",
      lines: vec![
        line(
          1,
          "Backup",
          Some("( 20.00% of instance price)"),
          "%",
          20.0,
          7.99,
        ),
        line(1, "RX41 Cloud Server", None, "Months", 1.0, 7.99),
        line(1, "RX22 Cloud Server", None, "Months", 1.0, 4.49),
        line(2, "Primary IPv4", None, "Months", 2.0, 0.5),
        line(4, "Snapshot", None, "GB-months", 8.2903, 0.0143),
        line(
          2,
          "TB add. Traffic (20 TB incl. traffic)",
          None,
          "TB",
          0.0,
          1.0,
        ),
        line(1, "Load Balancer LB11", None, "Months", 1.0, 5.39),
        line(3, "Volume 40 GB", None, "Months", 3.0, 1.91),
      ],
    },
    Project {
      name: "atlas-staging",
      period: "07/2026",
      lines: vec![
        line(2, "RX23 Cloud Server", None, "Months", 2.0, 3.99),
        line(1, "Primary IPv4", None, "Months", 1.0, 0.5),
        line(3, "Snapshot", None, "GB-months", 5.3534, 0.0143),
        line(
          1,
          "TB add. Traffic (20 TB incl. traffic)",
          None,
          "TB",
          0.0,
          1.0,
        ),
      ],
    },
    Project {
      name: "atlas-batch",
      period: "07/2026",
      lines: vec![
        line(6, "RX33 Cloud Server", None, "Months", 6.0, 5.99),
        line(6, "Primary IPv4", None, "Months", 6.0, 0.5),
        line(2, "Volume 80 GB", None, "Months", 2.0, 3.83),
        line(8, "Snapshot", None, "GB-months", 41.208, 0.0143),
        line(2, "Load Balancer LB11", None, "Months", 2.0, 5.39),
        line(
          6,
          "TB add. Traffic (20 TB incl. traffic)",
          None,
          "TB",
          1.62,
          1.0,
        ),
        line(
          1,
          "Backup",
          Some("( 20.00% of instance price)"),
          "%",
          20.0,
          5.99,
        ),
        line(2, "Floating IPv4", None, "Months", 2.0, 4.34),
      ],
    },
    Project {
      name: "atlas-analytics",
      period: "07/2026",
      lines: vec![
        line(1, "RX52 Cloud Server", None, "Months", 1.0, 12.49),
        line(2, "Volume 120 GB", None, "Months", 2.0, 5.74),
        line(
          1,
          "Backup",
          Some("( 20.00% of instance price)"),
          "%",
          20.0,
          12.49,
        ),
        line(5, "Snapshot", None, "GB-months", 22.118, 0.0143),
        line(1, "Floating IPv4", None, "Months", 1.0, 4.34),
        line(
          1,
          "TB add. Traffic (20 TB incl. traffic)",
          None,
          "TB",
          0.14,
          1.0,
        ),
      ],
    },
  ]
}

const CELL_RIGHT: &str = "border-left: 1px solid #1c1917; padding: 5px 8px;";

fn table_rows() -> String {
  let mut out = String::new();

  for project in projects() {
    let header = format!(
      r##"<div style="display: flex; border-top: 1px solid #1c1917; font-size: 12px; font-weight: 700; padding: 5px 8px;">Project "{}" ({})</div>"##,
      project.name, project.period
    );
    let mut rows: Vec<String> = Vec::new();
    let mut subtotal = 0.0_f32;

    for line in &project.lines {
      let amount = line_amount(line);

      subtotal += amount;
      let stripe = if line.pos % 2 == 1 {
        "background-color: #d9d9d9;"
      } else {
        ""
      };
      let detail = line
        .detail
        .map(|d| format!(r##"<span>{d}</span>"##))
        .unwrap_or_default();

      rows.push(format!(
        r##"<div style="display: flex; border-top: 1px solid #1c1917; font-size: 12px; {stripe}">
          <div style="width: 44px; text-align: right; padding: 5px 8px;">{pos}</div>
          <div style="width: 74px; text-align: right; {CELL_RIGHT}">{count}</div>
          <div style="flex: 1; display: flex; flex-direction: column; {CELL_RIGHT}">
            <span>{product}</span>{detail}
          </div>
          <div style="width: 88px; {CELL_RIGHT}">{unit}</div>
          <div style="width: 82px; text-align: right; {CELL_RIGHT}">{quantity}</div>
          <div style="width: 92px; text-align: right; {CELL_RIGHT}">€ {unit_price:.4}</div>
          <div style="width: 106px; text-align: right; {CELL_RIGHT}">€ {amount:.4}</div>
        </div>"##,
        pos = line.pos,
        count = line.count,
        product = line.product,
        unit = line.unit,
        quantity = line.quantity,
        unit_price = line.unit_price,
      ));
    }

    let subtotal_row = format!(
      r##"<div style="display: flex; justify-content: flex-end; border-top: 1px solid #1c1917; font-size: 12px; font-weight: 700;">
        <div style="padding: 5px 8px;">Subtotal (excl. VAT)</div>
        <div style="width: 106px; text-align: right; {CELL_RIGHT}">€ {subtotal:.2}</div>
      </div>"##
    );

    // The section header stays with its first row, and the subtotal with the
    // last row, so a page cut never strands either alone.
    let first = rows.remove(0);
    let last = rows.pop();

    out.push_str(&format!(
      r##"<div style="display: flex; flex-direction: column; break-inside: avoid;">{header}{first}</div>"##
    ));
    for row in &rows {
      out.push_str(row);
    }
    match last {
      Some(last) => out.push_str(&format!(
        r##"<div style="display: flex; flex-direction: column; break-inside: avoid;">{last}{subtotal_row}</div>"##
      )),
      None => out.push_str(&subtotal_row),
    }
  }
  out
}

const INVOICE_FOOTER: &str = r##"<div style="display: flex; flex-direction: column; width: 100%; padding: 0 36px; font-size: 8.5px; color: #1c1917;">
  <div style="display: flex; justify-content: flex-end; padding-bottom: 10px;"><span>{page} / {pages}</span></div>
  <div style="display: flex; gap: 24px; padding-top: 10px; border-top: 1px solid #9ca3af;">
    <div style="flex: 1; display: flex; flex-direction: column; gap: 1px;">
      <span style="font-weight: 700;">Ridgeline Cloud GmbH</span>
      <span>CEO: Mara Lindqvist,</span>
      <span>Jonas Weber, Priya Nair</span>
      <span>Freiburg Registration Office: HRB 71442</span>
      <span>VAT Reg. No.: DE318271644</span>
    </div>
    <div style="flex: 1; display: flex; flex-direction: column; gap: 1px;">
      <span>Bergwerkstr. 11</span>
      <span>79098 Freiburg | Germany</span>
      <span>Tel.: +49 761 4405-0</span>
      <span>Fax: +49 761 4405-19</span>
      <span>billing@ridgeline.example | www.ridgeline.example</span>
    </div>
    <div style="flex: 1; display: flex; flex-direction: column; gap: 1px;">
      <span>Bank details:</span>
      <span>Commerzbank AG Freiburg</span>
      <span>IBAN: DE89 6808 0030 0110 2334 00</span>
      <span>BIC: COBADEFFXXX</span>
    </div>
  </div>
</div>"##;

fn line_amount(line: &Line) -> f32 {
  if line.unit == "%" {
    line.unit_price * line.quantity / 100.0 * line.count as f32
  } else {
    line.unit_price * line.quantity
  }
}

fn invoice_html() -> String {
  let rows = table_rows();
  let net: f32 = projects()
    .iter()
    .flat_map(|p| p.lines.iter())
    .map(line_amount)
    .sum();
  let vat = net * 0.19;
  let gross = net + vat;

  format!(
    r##"<div style="display: flex; flex-direction: column; width: 100%; color: #1c1917; padding: 8px 36px 0 36px;">

  <div style="display: flex; justify-content: flex-end;">
    <span style="font-size: 34px; font-weight: 800; letter-spacing: 2px; color: #d50c2d;">RIDGELINE</span>
  </div>

  <div style="display: flex; align-items: flex-start; margin-top: 10px;">
    <div style="flex: 1; height: 2px; background-color: #6b7280; margin-top: 0;"></div>
    <div style="width: 26px; height: 2px; background-color: #6b7280; transform: rotate(18deg); transform-origin: left top;"></div>
    <div style="width: 300px; height: 2px; background-color: #6b7280; margin-top: 8px;"></div>
  </div>

  <div style="display: flex; flex-direction: column; align-items: flex-end; gap: 1px; margin-top: 56px; font-size: 12px;">
    <span>Customer ID: K1846220317</span>
    <span>Invoice no.: 086001022594</span>
    <span>Invoice date: 28/07/2026</span>
  </div>

  <div style="display: flex; flex-direction: column; border: 1px solid #1c1917; border-top-width: 0; margin-top: 28px;">
    <div style="display: flex; border-top: 1px solid #1c1917; font-size: 12px; font-weight: 700;">
      <div style="width: 44px; text-align: right; padding: 5px 8px; display: flex; align-items: flex-end; justify-content: flex-end;">Pos</div>
      <div style="width: 74px; text-align: right; {CELL_RIGHT}">Product count</div>
      <div style="flex: 1; display: flex; align-items: flex-end; {CELL_RIGHT}">Product</div>
      <div style="width: 88px; display: flex; align-items: flex-end; {CELL_RIGHT}">Unit</div>
      <div style="width: 82px; text-align: right; display: flex; align-items: flex-end; justify-content: flex-end; {CELL_RIGHT}">Quantity</div>
      <div style="width: 92px; text-align: right; display: flex; align-items: flex-end; justify-content: flex-end; {CELL_RIGHT}">Unit Price</div>
      <div style="width: 106px; text-align: right; display: flex; align-items: flex-end; justify-content: flex-end; {CELL_RIGHT}">Price (excl. VAT)</div>
    </div>
    {rows}
  </div>

  <div style="display: flex; justify-content: flex-end; margin: 14px 0 40px 0; break-inside: avoid;">
    <div style="width: 300px; display: flex; flex-direction: column; font-size: 12px;">
      <div style="display: flex; justify-content: space-between; padding: 3px 8px;"><span>Total (excl. VAT)</span><span>€ {net:.2}</span></div>
      <div style="display: flex; justify-content: space-between; padding: 3px 8px;"><span>VAT 19.00%</span><span>€ {vat:.2}</span></div>
      <div style="display: flex; justify-content: space-between; padding: 5px 8px; border-top: 1px solid #1c1917; font-weight: 700;"><span>Invoice amount</span><span>€ {gross:.2}</span></div>
    </div>
  </div>
</div>"##
  )
}

const CERTIFICATE: &str = r##"<div style="display: flex; width: 100%; height: 100%; background-color: #fcfbf8; padding: 40px;">
  <div style="flex: 1; display: flex; border: 2px solid #1c1917; padding: 5px;">
    <div style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; border: 1px solid #1c1917; padding: 48px 72px;">
      <span style="font-size: 11px; letter-spacing: 5px; color: #57534e;">NORTHWIND INSTITUTE OF DESIGN</span>
      <div style="width: 48px; height: 1px; background-color: #1c1917; margin: 22px 0;"></div>
      <span style="font-size: 34px; font-weight: 300; letter-spacing: 12px; color: #1c1917;">CERTIFICATE</span>
      <span style="font-size: 11px; letter-spacing: 4px; color: #57534e; margin-top: 6px;">OF COMPLETION</span>
      <span style="font-size: 11px; color: #78716c; margin-top: 34px;">This is to certify that</span>
      <span style="font-size: 38px; font-weight: 600; color: #1c1917; margin-top: 8px;">Alex Chen</span>
      <div style="width: 300px; height: 1px; background-color: #a8a29e; margin-top: 6px;"></div>
      <span style="font-size: 11.5px; color: #44403c; text-align: center; max-width: 520px; margin-top: 22px;">has successfully completed the Advanced Systems Rendering programme, comprising twelve weeks of coursework and a final capstone project, and is hereby recognized for outstanding achievement.</span>
      <div style="display: flex; gap: 140px; margin-top: 44px;">
        <div style="display: flex; flex-direction: column; align-items: center; gap: 5px;">
          <span style="font-size: 12px; color: #1c1917;">August 2, 2026</span>
          <div style="width: 170px; height: 1px; background-color: #1c1917;"></div>
          <span style="font-size: 8.5px; letter-spacing: 2px; color: #78716c;">DATE</span>
        </div>
        <div style="display: flex; flex-direction: column; align-items: center; gap: 5px;">
          <span style="font-size: 12px; color: #1c1917;">Dr. Renata Okafor, Dean</span>
          <div style="width: 170px; height: 1px; background-color: #1c1917;"></div>
          <span style="font-size: 8.5px; letter-spacing: 2px; color: #78716c;">SIGNATURE</span>
        </div>
      </div>
      <span style="font-size: 8.5px; letter-spacing: 2px; color: #a8a29e; margin-top: 36px;">CERTIFICATE NO. 2026-0117 · ISSUED IN PORTLAND, OREGON</span>
    </div>
  </div>
</div>"##;

fn invoice() -> Node {
  html(&invoice_html())
}
