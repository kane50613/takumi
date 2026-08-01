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

fn invoice_rows() -> String {
  let items = [
    ("Brand identity refresh", "Design", 1, 2400.0),
    ("Landing page build", "Engineering", 1, 3200.0),
    ("OG image pipeline setup", "Engineering", 1, 1450.0),
    ("Design system tokens", "Design", 1, 980.0),
    ("Marketing site copywriting", "Content", 6, 240.0),
    ("Blog template set", "Design", 3, 320.0),
    ("Email template suite", "Design", 4, 260.0),
    ("Checkout flow audit", "Research", 1, 1200.0),
    ("Accessibility review", "Research", 1, 890.0),
    ("Performance tuning sprint", "Engineering", 2, 1100.0),
    ("Analytics dashboard", "Engineering", 1, 2750.0),
    ("Illustration pack", "Design", 12, 85.0),
    ("Documentation portal", "Engineering", 1, 1980.0),
    ("Component library QA", "Engineering", 2, 640.0),
    ("Social media kit", "Design", 1, 540.0),
    ("Onboarding walkthrough", "Design", 1, 760.0),
  ];

  items
    .iter()
    .enumerate()
    .map(|(index, (name, team, qty, rate))| {
      let stripe = if index % 2 == 0 { "#ffffff" } else { "#f8fafc" };
      let amount = *qty as f32 * rate;

      format!(
        r##"<div style="display: flex; padding: 10px 14px; background-color: {stripe}; border-bottom: 1px solid #f1f5f9; font-size: 12px; color: #334155;">
          <div style="width: 300px; display: flex; flex-direction: column;">
            <span style="font-weight: 600; color: #0f172a;">{name}</span>
            <span style="font-size: 10px; color: #94a3b8;">{team}</span>
          </div>
          <div style="width: 60px; text-align: right;">{qty}</div>
          <div style="width: 110px; text-align: right;">${rate:.2}</div>
          <div style="width: 110px; text-align: right; font-weight: 600; color: #0f172a;">${amount:.2}</div>
        </div>"##
      )
    })
    .collect()
}

const INVOICE_FOOTER: &str = r##"<div style="display: flex; width: 100%; justify-content: space-between; padding: 12px 48px; font-size: 10px; color: #94a3b8; border-top: 1px solid #e2e8f0;">
  <span>Northwind Studio · hello@northwind.example</span>
  <span>Page {page} of {pages}</span>
</div>"##;

fn invoice_html() -> String {
  let rows = invoice_rows();

  format!(
    r##"<div style="display: flex; flex-direction: column; width: 100%; background-color: #ffffff; color: #0f172a;">
  <div style="height: 8px; background-image: linear-gradient(90deg, #4f46e5, #9333ea, #ec4899);"></div>

  <div style="display: flex; justify-content: space-between; align-items: flex-start; padding: 40px 48px 24px 48px;">
    <div style="display: flex; flex-direction: column; gap: 6px;">
      <div style="display: flex; align-items: center; gap: 10px;">
        <div style="width: 34px; height: 34px; border-radius: 10px; background-image: linear-gradient(135deg, #4f46e5, #9333ea); display: flex; align-items: center; justify-content: center; color: #ffffff; font-weight: 700; font-size: 16px;">N</div>
        <span style="font-size: 20px; font-weight: 700;">Northwind Studio</span>
      </div>
      <span style="font-size: 11px; color: #64748b;">128 Harbor Lane, Suite 4 · Portland, OR</span>
    </div>
    <div style="display: flex; flex-direction: column; align-items: flex-end; gap: 2px;">
      <span style="font-size: 26px; font-weight: 700; color: #4f46e5;">INVOICE</span>
      <span style="font-size: 12px; color: #64748b;">#2026-0142</span>
      <span style="font-size: 12px; color: #64748b;">Issued Aug 2, 2026 · Due Sep 1, 2026</span>
    </div>
  </div>

  <div style="display: flex; gap: 24px; padding: 0 48px 24px 48px;">
    <div style="flex: 1; display: flex; flex-direction: column; gap: 4px; padding: 14px 16px; background-color: #f8fafc; border-radius: 10px; border: 1px solid #e2e8f0;">
      <span style="font-size: 10px; font-weight: 700; color: #94a3b8;">BILLED TO</span>
      <span style="font-size: 13px; font-weight: 600;">Acme Robotics Inc.</span>
      <span style="font-size: 11px; color: #64748b;">finance@acme.example · 501 Mission St, San Francisco, CA</span>
    </div>
    <div style="width: 220px; display: flex; flex-direction: column; gap: 4px; padding: 14px 16px; background-color: #eef2ff; border-radius: 10px; border: 1px solid #c7d2fe;">
      <span style="font-size: 10px; font-weight: 700; color: #6366f1;">AMOUNT DUE</span>
      <span style="font-size: 22px; font-weight: 700; color: #312e81;">$21,470.00</span>
      <span style="font-size: 10px; color: #6366f1;">Net 30 · Wire or ACH</span>
    </div>
  </div>

  <div style="display: flex; flex-direction: column; margin: 0 48px; border: 1px solid #e2e8f0; border-radius: 10px; overflow: hidden;">
    <div style="display: flex; padding: 10px 14px; background-color: #0f172a; color: #e2e8f0; font-size: 10px; font-weight: 700;">
      <div style="width: 300px;">DESCRIPTION</div>
      <div style="width: 60px; text-align: right;">QTY</div>
      <div style="width: 110px; text-align: right;">RATE</div>
      <div style="width: 110px; text-align: right;">AMOUNT</div>
    </div>
    {rows}
  </div>

  <div style="display: flex; justify-content: flex-end; padding: 20px 48px; break-inside: avoid;">
    <div style="width: 280px; display: flex; flex-direction: column; gap: 6px; font-size: 12px; color: #334155;">
      <div style="display: flex; justify-content: space-between;"><span>Subtotal</span><span>$19,880.00</span></div>
      <div style="display: flex; justify-content: space-between;"><span>Tax (8%)</span><span>$1,590.00</span></div>
      <div style="display: flex; justify-content: space-between; padding-top: 8px; border-top: 2px solid #0f172a; font-size: 15px; font-weight: 700; color: #0f172a;"><span>Total</span><span>$21,470.00</span></div>
    </div>
  </div>

  <div style="margin: 0 48px 40px 48px; padding: 14px 16px; background-color: #f8fafc; border-radius: 10px; font-size: 11px; color: #64748b;">
    Thank you for your business. Please include the invoice number in your payment reference. Late payments accrue 1.5% monthly interest.
  </div>
</div>"##
  )
}

const CERTIFICATE: &str = r##"<div style="display: flex; width: 100%; height: 100%; background-image: linear-gradient(135deg, #fdfbf7, #f5efe0); padding: 28px;">
  <div style="flex: 1; display: flex; border: 3px solid #b45309; border-radius: 6px; padding: 6px;">
    <div style="flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; border: 1px solid #d9a441; border-radius: 3px; padding: 40px 60px; gap: 10px;">
      <div style="width: 64px; height: 64px; border-radius: 50%; background-image: radial-gradient(circle, #f59e0b, #b45309); display: flex; align-items: center; justify-content: center; color: #ffffff; font-size: 26px; font-weight: 700;">A</div>
      <span style="font-size: 13px; letter-spacing: 6px; color: #92400e; font-weight: 600;">CERTIFICATE OF COMPLETION</span>
      <span style="font-size: 12px; color: #78716c; margin-top: 12px;">This certificate is proudly presented to</span>
      <span style="font-size: 44px; font-weight: 700; color: #1c1917;">Alex Chen</span>
      <span style="font-size: 13px; color: #57534e; text-align: center; max-width: 560px;">for successfully completing the Advanced Systems Rendering course, demonstrating outstanding dedication across 12 weeks of coursework and a final capstone project.</span>
      <div style="display: flex; gap: 120px; margin-top: 36px;">
        <div style="display: flex; flex-direction: column; align-items: center; gap: 4px;">
          <span style="font-size: 15px; font-weight: 600; color: #1c1917;">Aug 2, 2026</span>
          <div style="width: 160px; height: 1px; background-color: #a8a29e;"></div>
          <span style="font-size: 10px; color: #78716c;">DATE</span>
        </div>
        <div style="display: flex; flex-direction: column; align-items: center; gap: 4px;">
          <span style="font-size: 15px; font-weight: 600; color: #1c1917;">Dr. R. Okafor</span>
          <div style="width: 160px; height: 1px; background-color: #a8a29e;"></div>
          <span style="font-size: 10px; color: #78716c;">INSTRUCTOR</span>
        </div>
      </div>
    </div>
  </div>
</div>"##;

fn invoice() -> Node {
  html(&invoice_html())
}
