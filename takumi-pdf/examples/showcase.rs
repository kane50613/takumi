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
      .page(PageOptions::a4().with_margin(0.0))
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

fn invoice_items() -> Vec<(&'static str, &'static str, u32, f32)> {
  vec![
    ("Brand identity refresh", "Fixed fee", 1, 2400.0),
    ("Landing page build", "Fixed fee", 1, 3200.0),
    ("OG image pipeline setup", "Fixed fee", 1, 1450.0),
    ("Design system tokens", "Fixed fee", 1, 980.0),
    ("Marketing site copywriting", "Per page", 6, 240.0),
    ("Blog template set", "Per template", 3, 320.0),
    ("Email template suite", "Per template", 4, 260.0),
    ("Checkout flow audit", "Fixed fee", 1, 1200.0),
    ("Accessibility review", "Fixed fee", 1, 890.0),
    ("Performance tuning sprint", "Per sprint", 2, 1100.0),
    ("Analytics dashboard", "Fixed fee", 1, 2750.0),
    ("Illustration pack", "Per piece", 12, 85.0),
    ("Documentation portal", "Fixed fee", 1, 1980.0),
    ("Component library QA", "Per sprint", 2, 640.0),
    ("Social media kit", "Fixed fee", 1, 540.0),
    ("Onboarding walkthrough", "Fixed fee", 1, 760.0),
    ("Print collateral templates", "Per template", 5, 180.0),
    ("Investor deck redesign", "Fixed fee", 1, 1650.0),
    ("Icon set expansion", "Per piece", 24, 45.0),
    ("Localization pass (ja, de)", "Per locale", 2, 720.0),
    ("Newsletter automation setup", "Fixed fee", 1, 830.0),
    ("Quarterly maintenance retainer", "Per month", 3, 900.0),
    ("Photography art direction", "Per day", 2, 650.0),
    ("Launch-day support", "Per day", 1, 480.0),
  ]
}

fn invoice_rows(items: &[(&str, &str, u32, f32)]) -> String {
  items
    .iter()
    .map(|(name, basis, qty, rate)| {
      let amount = *qty as f32 * rate;

      format!(
        r##"<div style="display: flex; align-items: center; padding: 9px 0; border-bottom: 1px solid #e7e5e4; font-size: 11px; color: #1c1917;">
          <div style="flex: 1; display: flex; flex-direction: column;">
            <span>{name}</span>
            <span style="font-size: 9px; color: #78716c;">{basis}</span>
          </div>
          <div style="width: 60px; text-align: right;">{qty}</div>
          <div style="width: 110px; text-align: right;">{rate:.2}</div>
          <div style="width: 120px; text-align: right;">{amount:.2}</div>
        </div>"##
      )
    })
    .collect()
}

const INVOICE_FOOTER: &str = r##"<div style="display: flex; width: 100%; justify-content: space-between; align-items: center; padding: 14px 64px; font-size: 8.5px; color: #a8a29e; border-top: 1px solid #e7e5e4;">
  <span>Northwind Studio LLC · Registered in Oregon, USA · EIN 87-1234567</span>
  <span>Page {page} of {pages}</span>
</div>"##;

fn thousands(value: f32) -> String {
  let cents = (value * 100.0).round() as i64;
  let (whole, frac) = (cents / 100, cents % 100);
  let mut digits = whole.to_string();
  let mut grouped = String::new();

  while digits.len() > 3 {
    let tail = digits.split_off(digits.len() - 3);

    grouped = format!(",{tail}{grouped}");
  }
  format!("{digits}{grouped}.{frac:02}")
}

fn invoice_html() -> String {
  let items = invoice_items();
  let rows = invoice_rows(&items);
  let subtotal: f32 = items
    .iter()
    .map(|(_, _, qty, rate)| *qty as f32 * rate)
    .sum();
  let tax = subtotal * 0.08;
  let total = subtotal + tax;
  let subtotal = thousands(subtotal);
  let tax = thousands(tax);
  let total = thousands(total);

  format!(
    r##"<div style="display: flex; flex-direction: column; width: 100%; background-color: #ffffff; color: #1c1917; padding: 56px 64px 0 64px;">

  <div style="display: flex; justify-content: space-between; align-items: flex-end; padding-bottom: 18px; border-bottom: 2px solid #1c1917;">
    <div style="display: flex; flex-direction: column; gap: 3px;">
      <span style="font-size: 15px; font-weight: 700; letter-spacing: 3px;">NORTHWIND STUDIO</span>
      <span style="font-size: 9.5px; color: #57534e;">128 Harbor Lane, Suite 4, Portland, OR 97209 · +1 (503) 555-0148 · billing@northwindstudio.example</span>
    </div>
    <span style="font-size: 24px; font-weight: 300; letter-spacing: 6px;">INVOICE</span>
  </div>

  <div style="display: flex; padding: 18px 0; border-bottom: 1px solid #e7e5e4;">
    <div style="flex: 1; display: flex; flex-direction: column; gap: 3px;">
      <span style="font-size: 8.5px; letter-spacing: 1.5px; color: #78716c;">BILLED TO</span>
      <span style="font-size: 12px; font-weight: 600;">Acme Robotics Inc.</span>
      <span style="font-size: 10px; color: #57534e;">501 Mission Street, Floor 9</span>
      <span style="font-size: 10px; color: #57534e;">San Francisco, CA 94105</span>
      <span style="font-size: 10px; color: #57534e;">accounts-payable@acme.example</span>
    </div>
    <div style="display: flex; gap: 40px;">
      <div style="display: flex; flex-direction: column; gap: 10px;">
        <div style="display: flex; flex-direction: column; gap: 2px;">
          <span style="font-size: 8.5px; letter-spacing: 1.5px; color: #78716c;">INVOICE NO.</span>
          <span style="font-size: 11px;">2026-0142</span>
        </div>
        <div style="display: flex; flex-direction: column; gap: 2px;">
          <span style="font-size: 8.5px; letter-spacing: 1.5px; color: #78716c;">TERMS</span>
          <span style="font-size: 11px;">Net 30</span>
        </div>
      </div>
      <div style="display: flex; flex-direction: column; gap: 10px;">
        <div style="display: flex; flex-direction: column; gap: 2px;">
          <span style="font-size: 8.5px; letter-spacing: 1.5px; color: #78716c;">ISSUE DATE</span>
          <span style="font-size: 11px;">August 2, 2026</span>
        </div>
        <div style="display: flex; flex-direction: column; gap: 2px;">
          <span style="font-size: 8.5px; letter-spacing: 1.5px; color: #78716c;">DUE DATE</span>
          <span style="font-size: 11px;">September 1, 2026</span>
        </div>
      </div>
    </div>
  </div>

  <div style="display: flex; flex-direction: column; margin-top: 26px;">
    <div style="display: flex; padding-bottom: 7px; border-bottom: 1px solid #1c1917; font-size: 8.5px; letter-spacing: 1.5px; color: #57534e;">
      <div style="flex: 1;">DESCRIPTION</div>
      <div style="width: 60px; text-align: right;">QTY</div>
      <div style="width: 110px; text-align: right;">UNIT PRICE (USD)</div>
      <div style="width: 120px; text-align: right;">AMOUNT (USD)</div>
    </div>
    {rows}
  </div>

  <div style="display: flex; justify-content: flex-end; padding-top: 14px; break-inside: avoid;">
    <div style="width: 290px; display: flex; flex-direction: column; font-size: 11px; color: #1c1917;">
      <div style="display: flex; justify-content: space-between; padding: 4px 0;"><span style="color: #57534e;">Subtotal</span><span>{subtotal}</span></div>
      <div style="display: flex; justify-content: space-between; padding: 4px 0;"><span style="color: #57534e;">Sales tax (8.0%)</span><span>{tax}</span></div>
      <div style="display: flex; justify-content: space-between; margin-top: 6px; padding: 7px 0 2px 0; border-top: 1px solid #1c1917; font-weight: 700; font-size: 12.5px;">
        <span>Total due</span><span>USD {total}</span>
      </div>
      <div style="height: 1px; background-color: #1c1917; margin-top: 2px;"></div>
    </div>
  </div>

  <div style="display: flex; flex-direction: column; gap: 3px; margin: 30px 0 48px 0; padding-top: 14px; border-top: 1px solid #e7e5e4; break-inside: avoid;">
    <span style="font-size: 8.5px; letter-spacing: 1.5px; color: #78716c;">PAYMENT INSTRUCTIONS</span>
    <span style="font-size: 10px; color: #57534e;">First National Bank · Account 4402-118821 · Routing 123000848 · Reference: INV 2026-0142</span>
    <span style="font-size: 10px; color: #57534e;">Payment is due within 30 days of the issue date. Please reference the invoice number with your remittance.</span>
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
