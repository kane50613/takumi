export const invoice = {
  number: "INV-2026-0042",
  issuedAt: "2026-08-04",
  dueAt: "2026-09-03",
  currency: "EUR",
  seller: {
    name: "Takumi Werkstatt GmbH",
    address: "Rasterstraße 12, 10115 Berlin",
    country: "DE",
    vatId: "DE811907980",
  },
  buyer: {
    name: "Atelier Chromium SARL",
    address: "88 rue du Rendu, 75011 Paris",
    country: "FR",
    vatId: "FR40303265045",
  },
  items: [
    { description: "Glyph subsetting, per document", quantity: 1200, unitPrice: 0.4 },
    { description: "Archival conversion, PDF/A-3", quantity: 1, unitPrice: 890 },
    { description: "Structure tree audit, PDF/UA-1", quantity: 1, unitPrice: 1450 },
    { description: "Headless Chrome decommissioning", quantity: 1, unitPrice: 0 },
  ],
  taxRate: 0.19,
};

export type Invoice = typeof invoice;

export const money = (amount: number, currency: string) =>
  amount.toLocaleString("en-IE", { style: "currency", currency });

export const totals = (data: Invoice) => {
  const net = data.items.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
  const tax = net * data.taxRate;

  return { net, tax, gross: net + tax };
};

const amount = (value: number) => value.toFixed(2);

const escape = (value: string) =>
  value.replace(
    /[&<>"]/g,
    (character) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character] ?? character,
  );

/** Factur-X 1.0 MINIMUM profile, the CII payload every EU e-invoice reader looks for. */
export function facturXml(data: Invoice) {
  const { net, tax, gross } = totals(data);

  return `<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice
  xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
  xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
  xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocumentContext>
    <ram:GuidelineSpecifiedDocumentContextParameter>
      <ram:ID>urn:factur-x.eu:1p0:minimum</ram:ID>
    </ram:GuidelineSpecifiedDocumentContextParameter>
  </rsm:ExchangedDocumentContext>
  <rsm:ExchangedDocument>
    <ram:ID>${escape(data.number)}</ram:ID>
    <ram:TypeCode>380</ram:TypeCode>
    <ram:IssueDateTime>
      <udt:DateTimeString format="102">${data.issuedAt.replaceAll("-", "")}</udt:DateTimeString>
    </ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeAgreement>
      <ram:SellerTradeParty>
        <ram:Name>${escape(data.seller.name)}</ram:Name>
        <ram:PostalTradeAddress>
          <ram:CountryID>${escape(data.seller.country)}</ram:CountryID>
        </ram:PostalTradeAddress>
        <ram:SpecifiedTaxRegistration>
          <ram:ID schemeID="VA">${escape(data.seller.vatId)}</ram:ID>
        </ram:SpecifiedTaxRegistration>
      </ram:SellerTradeParty>
      <ram:BuyerTradeParty>
        <ram:Name>${escape(data.buyer.name)}</ram:Name>
        <ram:SpecifiedTaxRegistration>
          <ram:ID schemeID="VA">${escape(data.buyer.vatId)}</ram:ID>
        </ram:SpecifiedTaxRegistration>
      </ram:BuyerTradeParty>
    </ram:ApplicableHeaderTradeAgreement>
    <ram:ApplicableHeaderTradeDelivery />
    <ram:ApplicableHeaderTradeSettlement>
      <ram:InvoiceCurrencyCode>${escape(data.currency)}</ram:InvoiceCurrencyCode>
      <ram:SpecifiedTradeSettlementHeaderMonetarySummation>
        <ram:TaxBasisTotalAmount>${amount(net)}</ram:TaxBasisTotalAmount>
        <ram:TaxTotalAmount currencyID="${escape(data.currency)}">${amount(tax)}</ram:TaxTotalAmount>
        <ram:GrandTotalAmount>${amount(gross)}</ram:GrandTotalAmount>
        <ram:DuePayableAmount>${amount(gross)}</ram:DuePayableAmount>
      </ram:SpecifiedTradeSettlementHeaderMonetarySummation>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>
`;
}
